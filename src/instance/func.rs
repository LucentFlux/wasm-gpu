use std::marker::PhantomData;
use std::sync::Arc;

use crate::atomic_counter::AtomicU32Counter;
use crate::capabilities::CapabilityStore;
use crate::func::{FuncAccessible, FuncData, FuncInstance};
use crate::session::Session;
use crate::{impl_immutable_ptr, Caller, DeviceStoreSet, FuncRef, Val, WasmTyVec};
use futures::future::BoxFuture;
use futures::FutureExt;
use itertools::Itertools;
use perfect_derive::perfect_derive;

#[perfect_derive(Debug)]
pub struct FuncsInstance<T> {
    host_funcs: naga::Arena<Box<dyn HostFunc<T>>>,
    wasm_functions: Vec<FuncInstance<T>>,

    index_generator: AtomicU32Counter,
    cap_set: CapabilityStore,
}

impl<T> FuncsInstance<T> {
    pub fn new() -> Self {
        Self {
            host_funcs: naga::Arena::new(),
            wasm_functions: Vec::new(),
            index_generator: AtomicU32Counter::new(),
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
            .push_within_capacity(FuncInstance {
                func_data,
                // Imports have to be filled in later
                accessible: None,
            })
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

        instance.accessible = Some(accessible);
    }

    pub fn register_host<Params, Results, F>(&mut self, func: F) -> UntypedFuncPtr<T>
    where
        Params: WasmTyVec + 'static,
        Results: WasmTyVec + Send + 'static,
        for<'b> F: Send
            + Sync
            + Fn(Caller<'b, T>, Params) -> BoxFuture<'b, anyhow::Result<Results>>
            + 'static,
    {
        let _func = Box::new(TypedHostFn::<F, T, Params, Results> {
            func,
            _phantom: Default::default(),
        });

        unimplemented!();
    }
}

/// Used for type erasure on parameters and results of function
struct TypedHostFn<F, T, Params, Results> {
    func: F,
    _phantom: PhantomData<fn(T, Params) -> (T, Results)>,
}

trait HostFunc<T>: Send + Sync {
    fn call<'a>(
        &self,
        caller: Caller<'a, T>,
        args: Vec<Val>,
    ) -> BoxFuture<'a, anyhow::Result<Vec<Val>>>;
}

impl<F, T, Params, Results> HostFunc<T> for TypedHostFn<F, T, Params, Results>
where
    F: 'static + for<'b> Fn(Caller<'b, T>, Params) -> BoxFuture<'b, anyhow::Result<Results>>,
    Params: WasmTyVec + 'static,
    Results: WasmTyVec + Send + 'static,
    TypedHostFn<F, T, Params, Results>: Send + Sync,
{
    fn call<'a>(
        &self,
        caller: Caller<'a, T>,
        args: Vec<Val>,
    ) -> BoxFuture<'a, anyhow::Result<Vec<Val>>> {
        let typed_args = match Params::try_from_val_vec(&args) {
            Ok(v) => v,
            Err(e) => return Box::pin(async { Err(e) }),
        };

        return Box::pin(
            (self.func)(caller, typed_args)
                .then(async move |r| r.map(|result| result.to_val_vec())),
        );
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
