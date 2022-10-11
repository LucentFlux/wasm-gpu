use crate::instance::abstr::global::AbstractGlobalPtr;
use crate::instance::abstr::memory::AbstractMemoryPtr;
use crate::instance::abstr::table::AbstractTablePtr;
use crate::instance::func::{TypedFuncPtr, UntypedFuncPtr};
use crate::typed::WasmTyVec;
use crate::Backend;

pub struct NamedExtern<'a, B, T>
where
    B: Backend,
{
    pub module: &'a str,
    pub name: &'a str,
    pub ext: Extern<B, T>,
}

impl<'a, B, T> Clone for NamedExtern<'a, B, T> {
    fn clone(&self) -> Self {
        Self {
            module: self.module.clone(),
            name: self.name.clone(),
            ext: self.ext.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Extern<B, T>
where
    B: Backend,
{
    Func(UntypedFuncPtr<B, T>),
    Global(AbstractGlobalPtr<B, T>),
    Table(AbstractTablePtr<B, T>),
    Memory(AbstractMemoryPtr<B, T>),
}

impl<B, T> Clone for Extern<B, T>
where
    B: Backend,
{
    fn clone(&self) -> Self {
        match self {
            Self::Func(f) => Self::Func(f.clone()),
            Self::Global(g) => Self::Global(g.clone()),
            Self::Table(t) => Self::Table(t.clone()),
            Self::Memory(m) => Self::Memory(m.clone()),
        }
    }
}

impl<B, T> From<UntypedFuncPtr<B, T>> for Extern<B, T>
where
    B: Backend,
{
    fn from(f: UntypedFuncPtr<B, T>) -> Self {
        Self::Func(f)
    }
}

impl<B, T, Params, Results> From<TypedFuncPtr<B, T, Params, Results>> for Extern<B, T>
where
    B: Backend,
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    fn from(f: TypedFuncPtr<B, T, Params, Results>) -> Self {
        Self::Func(f.as_untyped())
    }
}

impl<B, T> From<AbstractMemoryPtr<B, T>> for Extern<B, T>
where
    B: Backend,
{
    fn from(m: AbstractMemoryPtr<B, T>) -> Self {
        Self::Memory(m)
    }
}

impl<B, T> From<AbstractGlobalPtr<B, T>> for Extern<B, T>
where
    B: Backend,
{
    fn from(g: AbstractGlobalPtr<B, T>) -> Self {
        Self::Global(g)
    }
}

impl<B, T> From<AbstractTablePtr<B, T>> for Extern<B, T>
where
    B: Backend,
{
    fn from(t: AbstractTablePtr<B, T>) -> Self {
        Self::Table(t)
    }
}
