use std::sync::Arc;

use crate::capabilities::CapabilityStore;
use crate::session::{OutputType, Session};
use crate::{impl_immutable_ptr, DeviceStoreSet};
use futures::future::BoxFuture;
use futures::FutureExt;
use itertools::Itertools;
use once_cell::sync::Lazy;
use wasm_gpu_funcgen::{FuncAccessible, FuncData, FuncUnit};
use wasm_types::{FuncRef, Val, WasmTyVec};
use wgpu::BufferAsyncError;
use wgpu_async::{AsyncQueue, OutOfMemoryError};
use wgpu_lazybuffers::MemorySystem;

#[derive(Debug, Clone)]
pub struct FuncsInstance {
    wasm_functions: Vec<FuncUnit>,
    cap_set: CapabilityStore,
}

static EMPTY_ACCESSABLE: Lazy<Arc<FuncAccessible>> =
    Lazy::new(|| Arc::new(FuncAccessible::empty()));

impl FuncsInstance {
    pub fn new() -> Self {
        Self {
            wasm_functions: Vec::new(),
            cap_set: CapabilityStore::new(0),
        }
    }

    pub fn reserve(&mut self, count: usize) {
        self.wasm_functions.reserve_exact(count);
        self.cap_set = self.cap_set.resize_ref(self.wasm_functions.capacity())
    }

    pub fn register_definition(&mut self, func_data: FuncData) -> UntypedFuncPtr {
        let ptr = self.wasm_functions.len();
        let ty = func_data.ty.clone();
        self.wasm_functions
            .push_within_capacity(FuncUnit {
                data: func_data,
                // Imports have to be filled in later
                accessible: Arc::clone(&EMPTY_ACCESSABLE),
            })
            .expect("calls to `reserve` should be made before registering");

        return UntypedFuncPtr::new(ptr, self.cap_set.get_cap(), ty);
    }

    pub fn link_function_imports(&mut self, ptr: &UntypedFuncPtr, accessible: Arc<FuncAccessible>) {
        assert!(self.cap_set.check(&ptr.cap));

        let instance = self
            .wasm_functions
            .get_mut(ptr.ptr)
            .expect("if the pointer is valid, the pointed value must exist");

        instance.accessible = accessible;
    }

    pub fn all_ptrs(&self) -> Vec<UntypedFuncPtr> {
        self.wasm_functions
            .iter()
            .enumerate()
            .map(|(ptr, func)| {
                let ty = func.data.ty.clone();
                UntypedFuncPtr::new(ptr, self.cap_set.get_cap(), ty)
            })
            .collect_vec()
    }

    pub fn get(&self, ptr: &UntypedFuncPtr) -> &FuncUnit {
        assert!(self.cap_set.check(&ptr.cap));

        self.wasm_functions
            .get(ptr.ptr)
            .expect("if ptr was valid, since `wasm_functions` is append only, item must exist")
    }

    pub(crate) fn assembleable(&self) -> wasm_gpu_funcgen::FuncsInstance {
        wasm_gpu_funcgen::FuncsInstance {
            wasm_functions: self.wasm_functions.clone(),
        }
    }
}

impl_immutable_ptr!(
pub struct UntypedFuncPtr {
    data...
    ty: wasmparser::FuncType,
}
);

impl UntypedFuncPtr {
    pub fn to_func_ref(&self) -> FuncRef {
        FuncRef::try_from(Some(self.ptr as u32))
            .expect("cannot have more than u32::MAX - 1 functions")
    }

    pub fn try_typed<Params: WasmTyVec, Results: WasmTyVec>(
        self,
    ) -> anyhow::Result<TypedFuncPtr<Params, Results>> {
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

    pub fn typed<Params: WasmTyVec, Results: WasmTyVec>(self) -> TypedFuncPtr<Params, Results> {
        self.try_typed().unwrap()
    }

    /// # Panics
    /// This function panics if:
    ///  - this function is not in the given store set
    ///  - the arguments given don't match the arguments that the function takes
    pub async fn call_all<'a>(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        stores: &'a mut DeviceStoreSet,
        args: impl IntoIterator<Item = Vec<Val>>,
    ) -> Result<BoxFuture<'a, OutputType>, OutOfMemoryError> {
        let args = args.into_iter().collect();

        let session = Session::new(stores, self.clone(), args);
        return session.run(memory_system, queue).await;
    }
}

// Typed function pointers should have their types checked before construction
impl_immutable_ptr!(
pub struct TypedFuncPtr<Params: WasmTyVec, Results: WasmTyVec> {
    data...
    ty: wasmparser::FuncType,
}
);

impl<Params: WasmTyVec, Results: WasmTyVec> TypedFuncPtr<Params, Results> {
    pub fn as_untyped(&self) -> UntypedFuncPtr {
        UntypedFuncPtr::new(self.ptr, self.cap, self.ty.clone())
    }

    /// # Panics
    /// This function panics if:
    ///  - this function is not in the given store set
    pub async fn call_all<'a>(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        stores: &'a mut DeviceStoreSet,
        args: impl IntoIterator<Item = Params>,
    ) -> Result<
        BoxFuture<'a, Result<Vec<Result<Results, wasmtime_environ::Trap>>, BufferAsyncError>>,
        OutOfMemoryError,
    > {
        let args = args.into_iter().map(|v| v.to_val_vec()).collect();

        let entry_func = self.as_untyped();
        let session = Session::new(stores, entry_func.clone(), args);

        let gpu_future = session.run(memory_system, queue).await?;
        let typed_gpu_future = gpu_future.map(|res| {
            res.map(|ret| {
                // For each successful result, type it
                ret.into_iter()
                    .map(|v| {
                        v.map(|v| {
                            Results::try_from_val_vec(&v).expect("type safety should ensure that casting function results to this typed pointer type always succeeds")
                        })
                    })
                    .collect_vec()
            })
        })
        .boxed();

        return Ok(typed_gpu_future);
    }
}
