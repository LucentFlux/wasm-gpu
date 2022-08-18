use crate::for_each_function_signature;
use anyhow::Error;
use std::fmt::{Display, Formatter};
use wasmtime::{Val, WasmParams};

pub(crate) trait WasmTyVal: WasmParams + Sized {
    fn try_from_val(v: Val) -> anyhow::Result<Self>;
    fn to_val(self: &Self) -> Val;
}

pub trait WasmTyVec: WasmParams + Sized {
    fn try_from_val_vec(v: Vec<Val>) -> anyhow::Result<Self>;
    fn to_val_vec(self: &Self) -> Vec<Val>;
}

#[derive(Debug)]
pub(crate) struct WasmTyVecError {}

impl Display for WasmTyVecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "wasmy type to vec error")
    }
}

impl std::error::Error for WasmTyVecError {}

macro_rules! impl_vec_base {
    ($t:ty as $wt:path) => {
        impl WasmTyVal for $t {
            #[inline(always)]
            fn try_from_val(v: Val) -> anyhow::Result<Self> {
                if let $wt(i) = v {
                    return Ok(i);
                }
                Err(WasmTyVecError {})?
            }

            #[inline(always)]
            fn to_val(self: &Self) -> Val {
                $wt(self.clone())
            }
        }
    };
}

impl_vec_base!(i32 as Val::I32);
impl_vec_base!(i64 as Val::I64);
impl_vec_base!(u32 as Val::F32);
impl_vec_base!(u64 as Val::F64);

impl<T> WasmTyVec for T
where
    T: WasmTyVal,
    (T,): WasmParams,
{
    #[inline(always)]
    fn try_from_val_vec(v: Vec<Val>) -> anyhow::Result<Self> {
        if let [t] = v.as_slice() {
            return Ok(T::try_from_val(t.clone())?);
        }

        return Err(Error::from(WasmTyVecError {}));
    }

    #[inline(always)]
    fn to_val_vec(self: &Self) -> Vec<Val> {
        return vec![self.to_val()];
    }
}

macro_rules! impl_vec_rec {
    (0) => {};

    ($n:tt $($t:ident)*) => {
        impl<$($t),*> WasmTyVec for ($($t,)*)
        where
            $(
                $t: WasmTyVal,
            )*
            ($($t,)*): WasmParams,
        {
            #[inline(always)]
            #[allow(non_snake_case)]
            fn try_from_val_vec(v: Vec<Val>) -> anyhow::Result<Self> {
                if let [$($t),*] = v.as_slice() {
                    return Ok((
                        $(
                            $t::try_from_val($t.clone())?,
                        )*
                    ));
                }

                return Err(Error::from(WasmTyVecError {}));
            }

            #[inline(always)]
            #[allow(non_snake_case)]
            fn to_val_vec(self: &Self) -> Vec<Val> {
                let ($($t,)*) = self;
                return vec![
                    $(
                        $t.to_val(),
                    )*
                ];
            }
        }
    };
}

for_each_function_signature!(impl_vec_rec);
