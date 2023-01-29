use std::sync::Arc;

use crate::capabilities::CapabilityStore;
use crate::func::{FuncAccessible, FuncData, FuncInstance, FuncUnit, HostFunc, TypedHostFn};
use crate::session::Session;
use crate::{impl_immutable_ptr, Caller, DeviceStoreSet, FuncRef, Val, WasmTyVec};
use futures::future::BoxFuture;
use futures::FutureExt;
use itertools::Itertools;
use perfect_derive::perfect_derive;

#[perfect_derive(Debug)]
pub struct FuncsInstance<T> {
    host_funcs: naga::Arena<Box<dyn HostFunc<T>>>,
    wasm_functions: Vec<FuncUnit<T>>,
    cap_set: CapabilityStore,
}

impl<T> FuncsInstance<T> {
    pub fn new() -> Self {
        Self {
            host_funcs: naga::Arena::new(),
            wasm_functions: Vec::new(),
            cap_set: CapabilityStore::new(0),
        }
    }

    pub fn reserve(&mut self, count: usize) {
        self.wasm_functions.reserve_exact(count);
        self.cap_set = self.cap_set.resize_ref(self.wasm_functions.capacity())
    }

    pub fn register_definition(&mut self, func_data: FuncData) -> UntypedFuncPtr<T> {
        let ptr = self.wasm_functions.len();
        let ty = func_data.ty.clone();
        self.wasm_functions
            .push_within_capacity(FuncUnit::LocalFunction(FuncInstance {
                func_data,
                // Imports have to be filled in later
                accessible: None,
            }))
            .expect("calls to `reserve` should be made before registering");

        return UntypedFuncPtr::new(ptr, self.cap_set.get_cap(), ty);
    }

    pub fn link_function_imports(
        &mut self,
        ptr: &UntypedFuncPtr<T>,
        accessible: Arc<FuncAccessible<T>>,
    ) {
        assert!(self.cap_set.check(&ptr.cap));

        let instance = self
            .wasm_functions
            .get_mut(ptr.ptr)
            .expect("if the pointer is valid, the pointed value must exist");

        match instance {
            FuncUnit::LocalFunction(instance) => instance.accessible = Some(accessible),
            FuncUnit::HostFunction { .. } => {
                panic!("can't link imports of host function - host functions don't have a module")
            }
        }
    }

    pub fn all_ptrs(&self) -> Vec<UntypedFuncPtr<T>> {
        self.wasm_functions
            .iter()
            .enumerate()
            .map(|(ptr, func)| {
                let ty = match func {
                    FuncUnit::LocalFunction(instance) => instance.func_data.ty.clone(),
                    FuncUnit::HostFunction { host_handle } => self
                        .host_funcs
                        .try_get(host_handle.clone())
                        .expect("every registered host function should exist in memory")
                        .ty(),
                };
                UntypedFuncPtr::new(ptr, self.cap_set.get_cap(), ty)
            })
            .collect_vec()
    }

    pub fn get(&self, ptr: &UntypedFuncPtr<T>) -> &FuncUnit<T> {
        assert!(self.cap_set.check(&ptr.cap));

        self.wasm_functions
            .get(ptr.ptr)
            .expect("if ptr was valid, since `wasm_functions` is append only, item must exist")
    }
}

impl<T: 'static> FuncsInstance<T> {
    pub fn register_host<Params, Results, F>(&mut self, func: F) -> UntypedFuncPtr<T>
    where
        Params: WasmTyVec + 'static,
        Results: WasmTyVec + Send + 'static,
        for<'b> F: Send
            + Sync
            + Fn(Caller<'b, T>, Params) -> BoxFuture<'b, anyhow::Result<Results>>
            + 'static,
    {
        self.reserve(1);

        let func = Box::new(TypedHostFn::from(func));
        let ty = func.ty();
        let host_handle = self.host_funcs.append(func, naga::Span::UNDEFINED);

        let ptr = self.wasm_functions.len();
        self.wasm_functions
            .push_within_capacity(FuncUnit::HostFunction { host_handle })
            .expect("a call is made to reserve");

        return UntypedFuncPtr::new(ptr, self.cap_set.get_cap(), ty);
    }
}

impl_immutable_ptr!(
pub struct UntypedFuncPtr<T> {
        data...
    ty: wasmparser::FuncType,
}
);

impl<T> UntypedFuncPtr<T> {
    pub fn to_func_ref(&self) -> FuncRef {
        FuncRef::from_u32(self.ptr as u32)
    }

    pub fn try_typed<Params: WasmTyVec, Results: WasmTyVec>(
        self,
    ) -> anyhow::Result<TypedFuncPtr<T, Params, Results>> {
        if !Params::VAL_TYPES.eq(self.ty.params()) {
            return Err(anyhow::anyhow!(
                "function pointer parameters were not the correct type, expected {:?} but got {:?}",
                Params::VAL_TYPES,
                self.ty.params()
            ));
        }
        if !Results::VAL_TYPES.eq(self.ty.results()) {
            return Err(anyhow::anyhow!(
                "function pointer results were not the correct type, expected {:?} but got {:?}",
                Results::VAL_TYPES,
                self.ty.results()
            ));
        }
        Ok(TypedFuncPtr::new(self.ptr, self.cap, self.ty))
    }

    pub fn typed<Params: WasmTyVec, Results: WasmTyVec>(self) -> TypedFuncPtr<T, Params, Results> {
        self.try_typed().unwrap()
    }

    /// # Panics
    /// This function panics if:
    ///  - the function pointer does not refer to a store set that the function is in
    pub fn call_all<'a>(
        &self,
        stores: &'a mut DeviceStoreSet<T>,
        args: impl IntoIterator<Item = Vec<Val>>,
    ) -> BoxFuture<'a, Vec<anyhow::Result<Vec<Val>>>> {
        let args = args.into_iter().collect();

        let session = Session::new(stores, self.clone(), args);
        return session.run().boxed();
    }
}

// Typed function pointers should have their types checked before construction
impl_immutable_ptr!(
pub struct TypedFuncPtr<T, Params: WasmTyVec, Results: WasmTyVec> {
        data...
        ty: wasmparser::FuncType,
}
);

impl<T, Params: WasmTyVec, Results: WasmTyVec> TypedFuncPtr<T, Params, Results> {
    pub fn as_untyped(&self) -> UntypedFuncPtr<T> {
        UntypedFuncPtr::new(self.ptr, self.cap, self.ty.clone())
    }

    /// # Panics
    /// This function panics if:
    ///  - the function pointer does not refer to the store_set set
    pub fn call_all<'a>(
        &self,
        stores: &'a mut DeviceStoreSet<T>,
        args: impl IntoIterator<Item = Params>,
    ) -> BoxFuture<'a, Vec<anyhow::Result<Results>>> {
        let args = args.into_iter().map(|v| v.to_val_vec()).collect();

        let entry_func = self.as_untyped();
        let session = Session::new(stores, entry_func.clone(), args);
        return session
            .run()
            .map(|res| {
                // For each successful result, type it
                res.into_iter()
                    .map(|v| v.and_then(|v| Results::try_from_val_vec(&v)))
                    .collect_vec()
            })
            .boxed();
    }
}
