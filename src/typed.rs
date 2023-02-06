use anyhow::Error;
use std::fmt::{Display, Formatter};
use wasmparser::ValType;
use wast::core::{HeapType, WastArgCore};
use wast::WastArg;

pub const fn wasm_ty_bytes(ty: ValType) -> u8 {
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
pub struct Ieee32(u32);

impl Ieee32 {
    pub fn from_le_bytes(bs: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bs))
    }
    pub fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
    pub fn from_bits(v: u32) -> Self {
        Self(v)
    }
    pub fn bits(&self) -> u32 {
        self.0
    }
    pub fn to_float(self) -> f32 {
        f32::from_le_bytes(self.to_le_bytes())
    }
}

impl From<wasmparser::Ieee32> for Ieee32 {
    fn from(v: wasmparser::Ieee32) -> Self {
        Ieee32::from_bits(v.bits())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Ieee64(u64);

impl Ieee64 {
    pub fn from_le_bytes(bs: [u8; 8]) -> Self {
        Self(u64::from_le_bytes(bs))
    }
    pub fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }
    pub fn from_bits(v: u64) -> Self {
        Self(v)
    }
    pub fn bits(&self) -> u64 {
        self.0
    }
    pub fn to_float(self) -> f64 {
        f64::from_le_bytes(self.to_le_bytes())
    }
}

impl From<wasmparser::Ieee64> for Ieee64 {
    fn from(v: wasmparser::Ieee64) -> Self {
        Ieee64::from_bits(v.bits())
    }
}

macro_rules! impl_ref {
    ($ident:ident) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $ident(u32);

        impl $ident {
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
            pub fn from(v: &Option<u32>) -> Self {
                match v {
                    None => Self::none(),
                    Some(v) => Self::from_u32(*v),
                }
            }
            pub fn is_none(&self) -> bool {
                self.0 == 0
            }
            pub fn is_some(&self) -> bool {
                !self.is_none()
            }
        }
    };
}

impl_ref!(FuncRef);
impl_ref!(ExternRef);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Val {
    I32(i32),
    I64(i64),
    F32(Ieee32),
    F64(Ieee64),
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

impl<'a> From<WastArg<'a>> for Val {
    fn from(value: WastArg<'a>) -> Self {
        match value {
            WastArg::Core(c) => Self::from(c),
            WastArg::Component(_) => panic!("component model not supported"),
        }
    }
}

impl<'a> From<WastArgCore<'a>> for Val {
    fn from(value: WastArgCore<'a>) -> Self {
        match value {
            WastArgCore::I32(i) => Self::I32(i),
            WastArgCore::I64(i) => Self::I64(i),
            WastArgCore::F32(f) => Self::F32(Ieee32::from_bits(f.bits)),
            WastArgCore::F64(f) => Self::F64(Ieee64::from_bits(f.bits)),
            WastArgCore::V128(v) => Self::V128(u128::from_le_bytes(v.to_le_bytes())),
            WastArgCore::RefNull(HeapType::Func) => Self::FuncRef(FuncRef::none()),
            WastArgCore::RefNull(HeapType::Extern) => Self::ExternRef(ExternRef::none()),
            WastArgCore::RefNull(ty) => panic!("null reference of type {:?} is not supported", ty),
            WastArgCore::RefExtern(v) => Self::ExternRef(ExternRef::from_u32(v)),
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
    fn byte_count() -> usize {
        Self::VAL_TYPES
            .into_iter()
            .map(|ty| usize::from(wasm_ty_bytes(*ty)))
            .sum()
    }
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

                let mut bsc = [0u8; std::mem::size_of::<$t>()];
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
impl_vec_base!(Ieee32 as F32);
impl_vec_base!(Ieee64 as F64);
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
                let mut index = 0;
                let v: Self = ( $(
                    {
                        let start = index;
                        let end = start + std::mem::size_of::<$t>();
                        index = end;

                        if index > bs.len() {
                            return Err(anyhow::anyhow!("too few bytes"));
                        }

                        $t::try_from_bytes(&bs[start..end])?
                    },
                )* );

                if index != bs.len() {
                    return Err(anyhow::anyhow!("too many bytes"));
                }

                return Ok(v);
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
for_each_function_signature!(impl_vec_rec);
