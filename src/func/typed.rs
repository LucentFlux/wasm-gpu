use crate::typed::WasmTyVec;
use crate::Func;
use anyhow::Context;
use itertools::Itertools;
use std::marker::PhantomData;
use std::sync::Mutex;

pub struct TypedFunc<Params, Results>
where
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    _a: PhantomData<fn(Params) -> Results>,
    func: Mutex<Func>, // Can be swapped out with an optimised version
}

impl<Params, Results> TypedFunc<Params, Results>
where
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    pub async fn call(
        &self,
        args: impl IntoIterator<Item = Params>,
    ) -> Vec<anyhow::Result<Results>> {
        let args = args.into_iter().map(|arg| arg.to_val_vec()).collect_vec();

        let lock = self.func.lock().expect("typed func mutex poisoned");

        let res = lock.call(args).await;

        let res = res
            .into_iter()
            .map(|r| {
                r.map(|v| {
                    Results::try_from_val_vec(v)
                        .expect("results didn't match type despite compile time checking: this is a bug in spirv-wasm")
                })
            })
            .collect_vec();

        return res;
    }
}

impl<Params, Results> TryFrom<Func> for TypedFunc<Params, Results>
where
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    type Error = anyhow::Error;

    fn try_from(func: Func) -> Result<Self, Self::Error> {
        Params::typecheck(func.ty.params()).context("type mismatch with parameters")?;
        Results::typecheck(func.ty.results()).context("type mismatch with results")?;

        return Ok(Self {
            _a: Default::default(),
            func: Mutex::new(func),
        });
    }
}
