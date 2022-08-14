use crate::typed::WaspParams;
use anyhow::Context;
use std::marker::PhantomData;
use wasmparser::FuncType;
use wasmtime::{WasmParams, WasmResults};

enum FuncKind {
    Export(ExportFunction),
    Host(Box<HostFunc>),
}

pub struct Func {
    kind: FuncKind,
    ty: FuncType,
}

pub struct TypedFunc<Params, Results>
where
    Params: WaspParams,
    Results: WasmResults,
{
    _a: PhantomData<fn(Params) -> Results>,
    func: Func,
}

impl<Params, Results> TryFrom<Func> for TypedFunc<Params, Results>
where
    Params: WaspParams,
    Results: WasmResults,
{
    type Error = anyhow::Error;

    fn try_from(func: Func) -> Result<Self, Self::Error> {
        Params::SingularType::typecheck(func.ty.params())
            .context("type mismatch with parameters")?;
        Results::typecheck(func.ty.results()).context("type mismatch with results")?;

        return Ok(Self {
            _a: Default::default(),
            func,
        });
    }
}

#[macro_export]
macro_rules! for_each_function_signature {
    ($mac:ident) => {
        $mac!(0);
        $mac!(1 A1);
        $mac!(2 A1 A2);
        $mac!(3 A1 A2 A3);
        $mac!(4 A1 A2 A3 A4);
        $mac!(5 A1 A2 A3 A4 A5);
        $mac!(6 A1 A2 A3 A4 A5 A6);
        $mac!(7 A1 A2 A3 A4 A5 A6 A7);
        $mac!(8 A1 A2 A3 A4 A5 A6 A7 A8);
        $mac!(9 A1 A2 A3 A4 A5 A6 A7 A8 A9);
        $mac!(10 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10);
        $mac!(11 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11);
        $mac!(12 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12);
        $mac!(13 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13);
        $mac!(14 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14);
        $mac!(15 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14 A15);
        $mac!(16 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14 A15 A16);
    };
}
