use crate::capabilities::CapabilityStore;
use crate::session::Session;
use crate::typed::{FuncRef, Val, WasmTyVec};
use crate::{impl_immutable_ptr, DeviceStoreSet, Func};
use anyhow::anyhow;
use futures::future::BoxFuture;
use futures::FutureExt;
use itertools::Itertools;
use perfect_derive::perfect_derive;
use wasmparser::Type;

#[perfect_derive(Debug)]
pub struct FuncsInstance<T> {
    /// Holds data that can later be copied into memory
    funcs: Vec<Func<T>>,

    cap_set: CapabilityStore,
}

impl<T> FuncsInstance<T> {
    pub fn new() -> Self {
        Self {
            funcs: Vec::new(),
            cap_set: CapabilityStore::new(0),
        }
    }

    pub fn register(&mut self, func: Func<T>) -> UntypedFuncPtr<T> {
        let ty = func.ty();
        let ptr = self.funcs.len();

        self.funcs.push(func);

        self.cap_set = self.cap_set.resize_ref(self.funcs.len());

        return UntypedFuncPtr::new(ptr, self.cap_set.get_cap(), ty);
    }

    pub(crate) fn predict<'a>(
        &self,
        funcs: impl Iterator<Item = &'a Type>,
    ) -> Vec<UntypedFuncPtr<T>> {
        let start = self.funcs.len();
        funcs
            .enumerate()
            .map(|(i, f)| {
                UntypedFuncPtr::new(
                    start + i,
                    self.cap_set.get_cap(),
                    match f {
                        Type::Func(f) => f.clone(),
                    },
                )
            })
            .collect_vec()
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
            return Err(anyhow!(
                "function pointer parameters were not the correct type, expected {:?} but got {:?}",
                Params::VAL_TYPES,
                self.ty.params()
            ));
        }
        if !Results::VAL_TYPES.eq(self.ty.results()) {
            return Err(anyhow!(
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
        let session = Session::new(stores, entry_func, args);
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
