use crate::for_each_function_signature;
use anyhow::Error;
use std::fmt::{Display, Formatter};
use std::ops::{Add, Bound, Range, RangeBounds};
use wasmparser::ValType;

pub const fn wasm_ty_bytes(ty: ValType) -> usize {
    match ty {
        ValType::I32 => 4,
        ValType::I64 => 8,
        ValType::F32 => 4,
        ValType::F64 => 8,
        ValType::V128 => 16,
        ValType::FuncRef => 4,
        ValType::ExternRef => 4,
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct FuncRef(u32);

impl FuncRef {
    pub fn from_le_bytes(bs: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bs))
    }
    pub fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
    pub fn none() -> Self {
        Self(0)
    }
    pub fn from_u32(v: u32) -> Self {
        Self(v + 1)
    }
    pub fn as_u32(&self) -> Option<u32> {
        if self.0 == 0 {
            return None;
        }
        return Some(self.0 - 1);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ExternRef(u32);

impl ExternRef {
    pub fn from_le_bytes(bs: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bs))
    }
    pub fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
    pub fn none() -> Self {
        Self(0)
    }
    pub fn from_u32(v: u32) -> Self {
        Self(v + 1)
    }
    pub fn as_u32(&self) -> Option<u32> {
        if self.0 == 0 {
            return None;
        }
        return Some(self.0 - 1);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Val {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    V128(u128),
    FuncRef(FuncRef),
    ExternRef(ExternRef),
}

impl Val {
    pub const fn get_type(&self) -> ValType {
        match self {
            Val::I32(_) => ValType::I32,
            Val::I64(_) => ValType::I64,
            Val::F32(_) => ValType::F32,
            Val::F64(_) => ValType::F64,
            Val::V128(_) => ValType::V128,
            Val::FuncRef(_) => ValType::FuncRef,
            Val::ExternRef(_) => ValType::ExternRef,
        }
    }
}

pub trait WasmTyVal: Sized {
    const VAL_TYPE: ValType;
    fn try_from_val(v: Val) -> anyhow::Result<Self>;
    fn to_val(self: &Self) -> Val;
    fn try_from_bytes(bs: &[u8]) -> anyhow::Result<Self>;
    fn to_bytes(self: &Self) -> Vec<u8>;
}

pub trait WasmTyVec: Sized {
    const VAL_TYPES: &'static [ValType];
    fn try_from_val_vec(v: &Vec<Val>) -> anyhow::Result<Self>;
    fn to_val_vec(self: &Self) -> Vec<Val>;
    fn try_from_bytes(bs: &[u8]) -> anyhow::Result<Self>;
    fn to_bytes(self: &Self) -> Vec<u8>;
}

#[derive(Debug)]
pub(crate) struct WasmTyVecError {}

impl Display for WasmTyVecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "wasm type to vec error")
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

            #[inline(always)]
            fn try_from_bytes(bs: &[u8]) -> anyhow::Result<Self> {
                if bs.len() != std::mem::size_of::<$t>() {
                    return Err(Error::from(WasmTyVecError {}));
                }

                let bsc = [0u8; std::mem::size_of::<$t>()];
                bsc.copy_from_slice(bs);
                Ok(<$t>::from_le_bytes(bsc))
            }

            #[inline(always)]
            fn to_bytes(self: &Self) -> Vec<u8> {
                Vec::from(self.to_le_bytes())
            }
        }
    };
}

impl_vec_base!(i32 as I32);
impl_vec_base!(i64 as I64);
impl_vec_base!(f32 as F32);
impl_vec_base!(f64 as F64);
impl_vec_base!(u128 as V128);
impl_vec_base!(FuncRef as FuncRef);
impl_vec_base!(ExternRef as ExternRef);

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

    fn try_from_bytes(bs: &[u8]) -> anyhow::Result<Self> {
        if bs.len() == 0 {
            return Ok(());
        }

        return Err(Error::from(WasmTyVecError {}));
    }

    fn to_bytes(self: &Self) -> Vec<u8> {
        vec![]
    }
}

impl<T> WasmTyVec for T
where
    T: WasmTyVal,
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

    #[inline(always)]
    fn try_from_bytes(bs: &[u8]) -> anyhow::Result<Self> {
        T::try_from_bytes(bs)
    }

    #[inline(always)]
    fn to_bytes(self: &Self) -> Vec<u8> {
        <Self as WasmTyVal>::to_bytes(self)
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

            #[inline(always)]
            #[allow(non_snake_case)]
            fn try_from_bytes(bs: &[u8]) -> anyhow::Result<Self> {
                let mut iter = bs.into_iter();
                Ok(( $(
                    {
                        let mut part: Vec<u8> = Vec::new();
                        for _ in 0..std::mem::size_of::<$t>() {
                            part.push(iter.next().clone());
                        }
                        $t::try_from_bytes(part.as_slice())?
                    },
                )* ))
            }

            #[inline(always)]
            #[allow(non_snake_case)]
            fn to_bytes(self: &Self) -> Vec<u8> {
                let ($($t,)*) = self;
                let mut res = Vec::new();
                $(
                    let mut next_bytes = <$t as WasmTyVal>::to_bytes($t);
                    res.append(&mut next_bytes);
                )*
                return res;
            }
        }
    };
}

for_each_function_signature!(impl_vec_rec);

pub trait ToRange {
    type Value;

    const ZERO: Self::Value;

    fn half_open(&self, max: Self::Value) -> Range<Self::Value>;
}

impl<V: Default + Clone + Add<u8>, R: RangeBounds<V>> ToRange for R {
    type Value = V;
    const ZERO: Self::Value = V::default();

    fn half_open(&self, max: V) -> Range<V> {
        let start = match self.start_bound() {
            Bound::Included(b) => b.clone(),
            Bound::Excluded(b) => b.clone().add(1),
            Bound::Unbounded => Self::ZERO,
        };

        let end = match self.end_bound() {
            Bound::Included(b) => b.clone().add(1),
            Bound::Excluded(b) => b.clone(),
            Bound::Unbounded => max,
        };

        return Range { start, end };
    }
}
