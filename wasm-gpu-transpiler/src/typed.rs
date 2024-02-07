use nonmax::NonMaxU32;
use std::fmt::{Display, Formatter};
use wasmparser::{HeapType, RefType, ValType};
use wast::core::WastArgCore;
use wast::WastArg;

mod sealed {
    pub trait SealedValType {}
    impl SealedValType for wasmparser::ValType {}
}
pub trait ValTypeByteCount: sealed::SealedValType {
    fn byte_count(&self) -> u8;
    fn try_from_bytes(&self, bs: &[u8]) -> Result<Val, WasmTyValParseError>;
}
impl ValTypeByteCount for ValType {
    fn byte_count(&self) -> u8 {
        match self {
            ValType::I32 => 4,
            ValType::I64 => 8,
            ValType::F32 => 4,
            ValType::F64 => 8,
            ValType::V128 => 16,
            ValType::Ref(_) => 4,
        }
    }

    fn try_from_bytes(&self, bs: &[u8]) -> Result<Val, WasmTyValParseError> {
        match self {
            ValType::I32 => <i32 as WasmTyVal>::try_from_bytes(bs).map(Val::I32),
            ValType::I64 => <i64 as WasmTyVal>::try_from_bytes(bs).map(Val::I64),
            ValType::F32 => <f32 as WasmTyVal>::try_from_bytes(bs).map(Val::F32),
            ValType::F64 => <f64 as WasmTyVal>::try_from_bytes(bs).map(Val::F64),
            ValType::V128 => <V128 as WasmTyVal>::try_from_bytes(bs).map(Val::V128),
            ValType::Ref(rty) => match rty.heap_type() {
                HeapType::Func => <FuncRef as WasmTyVal>::try_from_bytes(bs).map(Val::FuncRef),
                HeapType::Extern => {
                    <ExternRef as WasmTyVal>::try_from_bytes(bs).map(Val::ExternRef)
                }
                _ => unimplemented!(),
            },
        }
    }
}

macro_rules! impl_ref {
    ($ident:ident) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $ident(Option<NonMaxU32>);

        impl $ident {
            pub fn from_le_bytes(bs: [u8; 4]) -> Self {
                Self(NonMaxU32::new(u32::from_le_bytes(bs)))
            }
            pub fn to_le_bytes(self) -> [u8; 4] {
                match self.as_u32() {
                    None => [0xFFu8; 4],
                    Some(v) => u32::to_le_bytes(v),
                }
            }
            pub fn none() -> Self {
                Self(None)
            }
            pub fn as_u32(&self) -> Option<u32> {
                self.0.map(|v| v.get())
            }
            pub fn is_null(&self) -> bool {
                self.0.is_none()
            }
            pub fn is_not_null(&self) -> bool {
                !self.0.is_none()
            }
        }

        impl From<Option<NonMaxU32>> for $ident {
            fn from(val: Option<NonMaxU32>) -> Self {
                Self(val)
            }
        }

        impl TryFrom<u32> for $ident {
            type Error = &'static str;
            fn try_from(val: u32) -> Result<Self, Self::Error> {
                if let Some(v) = NonMaxU32::new(val) {
                    return Ok(Self(Some(v)));
                } else {
                    return Err("4-byte reference type cannot hold u32::MAX");
                }
            }
        }

        impl TryFrom<Option<u32>> for $ident {
            type Error = &'static str;
            fn try_from(val: Option<u32>) -> Result<Self, Self::Error> {
                let val = match val {
                    None => return Ok(Self(None)),
                    Some(val) => val,
                };
                Self::try_from(val)
            }
        }
    };
}

impl_ref!(FuncRef);
impl_ref!(ExternRef);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct V128(u128);

impl V128 {
    pub fn from_le_bytes(bs: [u8; 16]) -> Self {
        Self(u128::from_le_bytes(bs))
    }
    pub fn to_le_bytes(self) -> [u8; 16] {
        self.0.to_le_bytes()
    }
    pub fn from_bits(v: u128) -> Self {
        Self(v)
    }
    pub fn bits(&self) -> u128 {
        self.0
    }
}

impl From<wasmparser::V128> for V128 {
    fn from(v: wasmparser::V128) -> Self {
        V128::from_bits(u128::from_le_bytes(v.i128().to_le_bytes()))
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Val {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    V128(V128),
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
            Val::FuncRef(_) => ValType::Ref(RefType::FUNCREF),
            Val::ExternRef(_) => ValType::Ref(RefType::EXTERNREF),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Val::I32(v) => WasmTyVal::to_bytes(v),
            Val::I64(v) => WasmTyVal::to_bytes(v),
            Val::F32(v) => WasmTyVal::to_bytes(v),
            Val::F64(v) => WasmTyVal::to_bytes(v),
            Val::V128(v) => WasmTyVal::to_bytes(v),
            Val::FuncRef(v) => WasmTyVal::to_bytes(v),
            Val::ExternRef(v) => WasmTyVal::to_bytes(v),
        }
    }

    /// Checks that the types and the binary representation of two values are the same.
    /// This is different to `Eq::eq` in that `F32(NaN) != F32(NaN)` but
    /// `F32(cannonical NaN) == F32(cannonical NaN)`
    pub fn bitwise_eq(&self, other: &Self) -> bool {
        self.get_type() == other.get_type() && self.to_bytes() == other.to_bytes()
    }
}

impl<'a> TryFrom<WastArg<'a>> for Val {
    type Error = String;

    fn try_from(value: WastArg<'a>) -> Result<Self, Self::Error> {
        match value {
            WastArg::Core(c) => Self::try_from(c),
            WastArg::Component(_) => Err("component model not supported".to_owned()),
        }
    }
}

impl<'a> TryFrom<WastArgCore<'a>> for Val {
    type Error = String;

    fn try_from(value: WastArgCore<'a>) -> Result<Self, Self::Error> {
        let res = match value {
            WastArgCore::I32(i) => Self::I32(i),
            WastArgCore::I64(i) => Self::I64(i),
            WastArgCore::F32(f) => Self::F32(f32::from_bits(f.bits)),
            WastArgCore::F64(f) => Self::F64(f64::from_bits(f.bits)),
            WastArgCore::V128(v) => Self::V128(V128::from_le_bytes(v.to_le_bytes())),
            WastArgCore::RefNull(wast::core::HeapType::Func) => Self::FuncRef(FuncRef::none()),
            WastArgCore::RefNull(wast::core::HeapType::Extern) => {
                Self::ExternRef(ExternRef::none())
            }
            WastArgCore::RefNull(ty) => {
                return Err(format!("null reference of type {:?} is not supported", ty))
            }
            WastArgCore::RefExtern(v) => match ExternRef::try_from(v) {
                Ok(v) => Self::ExternRef(v),
                Err(_) => return Err(format!("extern ref can't be u32::MAX")),
            },
            WastArgCore::RefHost(v) => return Err("HostRefs are not supported".to_owned()),
        };

        Ok(res)
    }
}

pub trait WasmTyVal: Sized {
    const VAL_TYPE: ValType;
    fn try_from_val(v: Val) -> Result<Self, WasmTyValCoercionError>;
    fn to_val(self: &Self) -> Val;
    fn try_from_bytes(bs: &[u8]) -> Result<Self, WasmTyValParseError>;
    fn to_bytes(self: &Self) -> Vec<u8>;
    fn byte_count() -> u8 {
        Self::VAL_TYPE.byte_count()
    }
}

pub trait WasmTyVec: Sized {
    const VAL_TYPES: &'static [ValType];
    fn try_from_val_vec(v: &Vec<Val>) -> Result<Self, WasmTyVecCoercionError>;
    fn to_val_vec(self: &Self) -> Vec<Val>;
    fn try_from_bytes(bs: &[u8]) -> Result<Self, WasmTyVecParseError>;
    fn to_bytes(self: &Self) -> Vec<u8>;
    fn byte_count() -> usize {
        Self::VAL_TYPES
            .into_iter()
            .map(|ty| usize::from(ty.byte_count()))
            .sum()
    }
}

#[derive(Debug)]
pub struct WasmTyValCoercionError {
    source: Val,
    target: ValType,
}
impl Display for WasmTyValCoercionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "wasm val {:?} could not be coersed into types {:?}",
            self.source, self.target
        )
    }
}
impl std::error::Error for WasmTyValCoercionError {}

#[derive(Debug)]
pub struct WasmTyValParseError {
    source: Vec<u8>,
    target: ValType,
}
impl Display for WasmTyValParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "bytes {:?} could not be parsed into type {:?}",
            self.source, self.target
        )
    }
}
impl std::error::Error for WasmTyValParseError {}

#[derive(Debug)]
pub struct WasmTyVecCoercionError {
    sources: Vec<Val>,
    targets: Vec<ValType>,
}
impl Display for WasmTyVecCoercionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "wasm values {:?} could not be coersed into types {:?}",
            self.sources, self.targets
        )
    }
}
impl std::error::Error for WasmTyVecCoercionError {}

#[derive(Debug)]
pub struct WasmTyVecParseError {
    source: Vec<u8>,
    targets: Vec<ValType>,
}
impl Display for WasmTyVecParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "bytes {:?} could not be parsed into types {:?}",
            self.source, self.targets
        )
    }
}
impl std::error::Error for WasmTyVecParseError {}

macro_rules! impl_vec_base {
    ($t:ty as $wt:tt ; $($at:tt)*) => {
        impl WasmTyVal for $t {
            const VAL_TYPE: ValType = ValType::$($at)*;

            #[inline(always)]
            fn try_from_val(v: Val) -> Result<Self, WasmTyValCoercionError> {
                if let Val::$wt(i) = v {
                    return Ok(i);
                }
                Err(WasmTyValCoercionError {
                    source: v,
                    target: Self::VAL_TYPE,
                })
            }

            #[inline(always)]
            fn to_val(self: &Self) -> Val {
                Val::$wt(self.clone())
            }

            #[inline(always)]
            fn try_from_bytes(bs: &[u8]) -> Result<Self, WasmTyValParseError> {
                if bs.len() != std::mem::size_of::<$t>() {
                    return Err(WasmTyValParseError {
                        source: Vec::from(bs),
                        target: Self::VAL_TYPE,
                    });
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

impl_vec_base!(i32 as I32; I32);
impl_vec_base!(i64 as I64; I64);
impl_vec_base!(f32 as F32; F32);
impl_vec_base!(f64 as F64; F64);
impl_vec_base!(V128 as V128; V128);
impl_vec_base!(FuncRef as FuncRef; Ref(RefType::FUNCREF));
impl_vec_base!(ExternRef as ExternRef; Ref(RefType::EXTERNREF));

impl WasmTyVec for () {
    const VAL_TYPES: &'static [ValType] = &[];

    #[inline(always)]
    fn try_from_val_vec(v: &Vec<Val>) -> Result<Self, WasmTyVecCoercionError> {
        if v.len() == 0 {
            return Ok(());
        }

        return Err(WasmTyVecCoercionError {
            sources: v.clone(),
            targets: Vec::from(Self::VAL_TYPES),
        });
    }

    #[inline(always)]
    fn to_val_vec(self: &Self) -> Vec<Val> {
        return vec![];
    }

    fn try_from_bytes(bs: &[u8]) -> Result<Self, WasmTyVecParseError> {
        if bs.len() == 0 {
            return Ok(());
        }

        return Err(WasmTyVecParseError {
            source: Vec::from(bs),
            targets: Vec::from(Self::VAL_TYPES),
        });
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
    fn try_from_val_vec(v: &Vec<Val>) -> Result<Self, WasmTyVecCoercionError> {
        if let [t] = v.as_slice() {
            if let Ok(v) = T::try_from_val(t.clone()) {
                return Ok(v);
            }
        }

        return Err(WasmTyVecCoercionError {
            sources: v.clone(),
            targets: Vec::from(Self::VAL_TYPES),
        });
    }

    #[inline(always)]
    fn to_val_vec(self: &Self) -> Vec<Val> {
        return vec![self.to_val()];
    }

    #[inline(always)]
    fn try_from_bytes(bs: &[u8]) -> Result<Self, WasmTyVecParseError> {
        T::try_from_bytes(bs).map_err(|_| WasmTyVecParseError {
            source: Vec::from(bs),
            targets: Vec::from(Self::VAL_TYPES),
        })
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
            fn try_from_val_vec(v: &Vec<Val>) -> Result<Self, WasmTyVecCoercionError> {
                if let [$($t),*] = v.as_slice() {
                    return Ok((
                        $(
                            $t::try_from_val($t.clone()).map_err(|_| WasmTyVecCoercionError {
                                sources: v.clone(),
                                targets: Vec::from(Self::VAL_TYPES),
                            })?,
                        )*
                    ));
                }

                return Err(WasmTyVecCoercionError {
                    sources: v.clone(),
                    targets: Vec::from(Self::VAL_TYPES),
                });
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
            fn try_from_bytes(bs: &[u8]) -> Result<Self, WasmTyVecParseError> {
                let mut index = 0;
                let v: Self = ( $(
                    {
                        let start = index;
                        let end = start + std::mem::size_of::<$t>();
                        index = end;

                        if index > bs.len() {
                            return Err(WasmTyVecParseError {
                                source: Vec::from(bs),
                                targets: Vec::from(Self::VAL_TYPES),
                            });
                        }

                        $t::try_from_bytes(&bs[start..end]).map_err(|_| WasmTyVecParseError {
                            source: Vec::from(bs),
                            targets: Vec::from(Self::VAL_TYPES),
                        })?
                    },
                )* );

                if index != bs.len() {
                    return Err(WasmTyVecParseError {
                        source: Vec::from(bs),
                        targets: Vec::from(Self::VAL_TYPES),
                    });
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
