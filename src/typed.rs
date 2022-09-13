use crate::for_each_function_signature;
use anyhow::Error;
use std::fmt::{Display, Formatter};
use wasmtime::{Val, ValType, WasmParams};

pub(crate) trait WasmTyVal: WasmParams + Sized {
    const VAL_TYPE: ValType;
    fn try_from_val(v: Val) -> anyhow::Result<Self>;
    fn to_val(self: &Self) -> Val;
}

pub trait WasmTyVec: WasmParams + Sized {
    const VAL_TYPES: &'static [ValType];
    fn try_from_val_vec(v: &Vec<Val>) -> anyhow::Result<Self>;
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
    ($t:ty as $wt:tt) => {
        impl WasmTyVal for $t {
            const VAL_TYPE: ValType = ValType::$wt;

            #[inline(always)]
            fn try_from_val(v: Val) -> anyhow::Result<Self> {
                if let Val::$wt(i) = v {
                    return Ok(i);
                }
                Err(WasmTyVecError {})?
            }

            #[inline(always)]
            fn to_val(self: &Self) -> Val {
                Val::$wt(self.clone())
            }
        }
    };
}

impl_vec_base!(i32 as I32);
impl_vec_base!(i64 as I64);
impl_vec_base!(u32 as F32);
impl_vec_base!(u64 as F64);

impl WasmTyVec for () {
    const VAL_TYPES: &'static [ValType] = &[];

    #[inline(always)]
    fn try_from_val_vec(v: &Vec<Val>) -> anyhow::Result<Self> {
        if v.len() == 0 {
            return Ok(());
        }

        return Err(Error::from(WasmTyVecError {}));
    }

    #[inline(always)]
    fn to_val_vec(self: &Self) -> Vec<Val> {
        return vec![];
    }
}

impl<T> WasmTyVec for T
where
    T: WasmTyVal,
    (T,): WasmParams,
{
    const VAL_TYPES: &'static [ValType] = &[T::VAL_TYPE];

    #[inline(always)]
    fn try_from_val_vec(v: &Vec<Val>) -> anyhow::Result<Self> {
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
            const VAL_TYPES: &'static [ValType] = &[$($t::VAL_TYPE),*];

            #[inline(always)]
            #[allow(non_snake_case)]
            fn try_from_val_vec(v: &Vec<Val>) -> anyhow::Result<Self> {
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
