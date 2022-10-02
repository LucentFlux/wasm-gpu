use crate::session::Session;
use crate::typed::{FuncRef, WasmTyVec};
use crate::{impl_ptr, Backend, Func, StoreSet};
use futures::future::BoxFuture;
use futures::FutureExt;
use itertools::Itertools;
use std::hash::{Hash, Hasher};
use wasmparser::{FuncType, Type};

pub struct FuncsInstance<B, T>
where
    B: Backend,
{
    /// Holds data that can later be copied into memory
    funcs: Vec<Func<B, T>>,

    store_id: usize,
}

impl<B, T> FuncsInstance<B, T>
where
    B: Backend,
{
    pub fn register(&mut self, func: Func<B, T>) -> AbstractUntypedFuncPtr<B, T> {
        let ty = func.ty();
        let ptr = self.funcs.len();

        self.funcs.push(func);

        return AbstractUntypedFuncPtr::new(ptr, self.store_id, ty);
    }

    pub(crate) fn predict(
        &self,
        funcs: impl Iterator<Item = &Type>,
    ) -> Vec<AbstractUntypedFuncPtr<B, T>> {
        let start = self.funcs.len();
        funcs
            .enumerate()
            .map(|(i, f)| {
                AbstractUntypedFuncPtr::new(
                    start + i,
                    self.store_id,
                    match f {
                        Type::Func(f) => f.clone(),
                    },
                )
            })
            .collect_vec()
    }
}

impl_ptr!(
    pub struct UntypedFuncPtr<B: Backend, T> {
        ...
        ty: FuncType,
    }

    impl<B: Backend, T> UntypedFuncPtr<B, T>
    {
        pub fn ty(&self) -> FuncType {
            return self.ty.clone();
        }

        pub fn to_func_ref(&self) -> FuncRef {
            FuncRef::from_u32(self.ptr as u32)
        }
    }
);

impl<B, T> AbstractUntypedFuncPtr<B, T>
where
    B: Backend,
{
    pub fn typed<Params: WasmTyVec, Results: WasmTyVec>(
        self,
    ) -> AbstractTypedFuncPtr<B, T, Params, Results> {
        Params::typecheck(self.ty.params()).expect(format!(
            "function pointer parameters were not the correct type, expected {:?} but got {:?}",
            Params::VAL_TYPES,
            self.ty.params()
        ));
        Results::typecheck(self.ty.results()).expect(format!(
            "function pointer results were not the correct type, expected {:?} but got {:?}",
            Results::VAL_TYPES,
            self.ty.results()
        ));
        AbstractTypedFuncPtr::new(self.ptr, self.store_id, self.ty)
    }

    /// # Panics
    /// This function panics if:
    ///  - the function pointer does not refer to the store set
    fn call_all<'a>(
        &self,
        stores: &'a mut StoreSet<B, T>,
        mut args_fn: impl FnMut(&T) -> Self::Params,
    ) -> BoxFuture<'a, Vec<anyhow::Result<Self::Results>>> {
        let args = stores.datas().map(args_fn).collect();

        let session = Session::new(stores.backend(), stores, entry_func, args);
        return session.run().boxed();
    }
}

// Typed function pointers should have their types checked before construction
impl_ptr!(
    pub struct TypedFuncPtr<B: Backend, T, Params: WasmTyVec, Results: WasmTyVec> {
        ...
        ty: FuncType,
    }

    impl<B: Backend, T, Params: WasmTyVec, Results: WasmTyVec> TypedFuncPtr<B, T, Params, Results>
    {
        pub fn ty(&self) -> FuncType {
            return self.ty.clone();
        }

        pub fn to_func_ref(&self) -> FuncRef {
            FuncRef(self.ptr as u32)
        }
    }
);

impl<B, T, Params, Results> AbstractTypedFuncPtr<B, T, Params, Results>
where
    B: Backend,
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    pub fn as_untyped(&self) -> AbstractUntypedFuncPtr<B, T> {
        AbstractUntypedFuncPtr::new(self.ptr, self.store_id, self.ty.clone())
    }

    /// # Panics
    /// This function panics if:
    ///  - the function pointer does not refer to the store set
    pub fn call_all<'a>(
        &self,
        stores: &'a mut StoreSet<B, T>,
        mut args_fn: impl FnMut(&T) -> Params,
    ) -> BoxFuture<'a, Vec<anyhow::Result<Results>>> {
        let args = stores
            .datas()
            .map(args_fn)
            .map(|v| v.to_val_vec())
            .collect_vec();

        let entry_func = self.as_untyped();
        let session = Session::new(stores.backend(), stores, entry_func, args);
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
