use crate::func::{AbstractFuncPtr, FuncSet};
use crate::typed::WasmTyVec;
use crate::{Backend, FuncPtr, StoreSet};
use anyhow::Context;
use futures::future::BoxFuture;
use futures::FutureExt;
use itertools::Itertools;
use std::marker::PhantomData;
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

impl<B, T, Params, Results> AbstractFuncPtr<B, T> for TypedFuncPtr<B, T, Params, Results>
where
    B: Backend,
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    type Params = Params;
    type Results = Results;

    fn parse_params(params: Self::Params) -> Vec<Val> {
        params.to_val_vec()
    }

    fn gen_results(results: Vec<Val>) -> anyhow::Result<Self::Results> {
        Results::try_from_val_vec(&results)
    }

    fn get_ptr(&self) -> FuncPtr<B, T> {
        self.func.get_ptr()
    }
}
