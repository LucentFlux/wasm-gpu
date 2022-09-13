use crate::func::MultiCallable;
use crate::typed::WasmTyVec;
use crate::{Backend, FuncPtr, StoreSet};
use anyhow::Context;
use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::FutureExt;
use itertools::Itertools;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Mutex;
use wasmtime::Val;

pub struct TypedFuncPtr<B, T, Params, Results>
where
    B: Backend,
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    _a: PhantomData<fn(Params) -> Results>,
    func: FuncPtr<B, T>,
}

impl<B, T, Params, Results> TryFrom<FuncPtr<B, T>> for TypedFuncPtr<B, T, Params, Results>
where
    B: Backend,
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    type Error = anyhow::Error;

    fn try_from(func: FuncPtr<B, T>) -> Result<Self, Self::Error> {
        Params::typecheck(func.params()).context("type mismatch with parameters")?;
        Results::typecheck(func.results()).context("type mismatch with results")?;

        return Ok(Self {
            _a: Default::default(),
            func,
        });
    }
}

impl<B, T, Params, Results> From<TypedFuncPtr<B, T, Params, Results>> for FuncPtr<B, T>
where
    B: Backend,
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    fn from(tfp: TypedFuncPtr<B, T, Params, Results>) -> Self {
        tfp.func
    }
}

pub trait TypedMultiCallable<'a, B, T, Params, Results>
where
    B: Backend,
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    /// A typed version of MultiCallable<B, T>
    fn call_all(
        self,
        stores: &'a mut StoreSet<B, T>,
        args_fn: impl FnMut(&T) -> Params,
    ) -> BoxFuture<'a, Vec<anyhow::Result<Results>>>;
}

impl<'a, B: 'a, T: 'a, Params, Results, V> TypedMultiCallable<'a, B, T, Params, Results> for V
where
    V: IntoIterator<Item = &'a TypedFuncPtr<B, T, Params, Results>>,
    B: Backend,
    Params: WasmTyVec + 'a,
    Results: WasmTyVec + 'a,
{
    fn call_all(
        self,
        stores: &'a mut StoreSet<B, T>,
        mut args_fn: impl FnMut(&T) -> Params,
    ) -> BoxFuture<'a, Vec<anyhow::Result<Results>>> {
        let funcs: Vec<&FuncPtr<B, T>> = self.into_iter().map(|v| &v.func).collect_vec();

        let futures = funcs
            .call_all(stores, move |v| args_fn(v).to_val_vec())
            .then(async move |results| {
                results
                    .into_iter()
                    .map_ok(move |res| {
                        Results::try_from_val_vec(&res)
                            .expect("typechecking failed, function returned invalid result")
                    })
                    .collect_vec()
            });
        return Box::pin(futures);
    }
}
