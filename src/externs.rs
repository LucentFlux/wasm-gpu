use crate::instance::func::{TypedFuncPtr, UntypedFuncPtr};
use crate::instance::global::builder::AbstractGlobalPtr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use crate::instance::table::builder::AbstractTablePtr;
use crate::typed::WasmTyVec;
use itertools::Itertools;
use lib_hal::backend::Backend;
use wasmparser::{FuncType, GlobalType, TableType};

pub struct NamedExtern<B, T>
where
    B: Backend,
{
    pub module: String,
    pub name: String,
    pub ext: Extern<B, T>,
}

impl<B: Backend, T> Clone for NamedExtern<B, T> {
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

impl<B, T> Extern<B, T>
where
    B: Backend,
{
    pub fn signature(&self) -> String {
        match self {
            Extern::Func(f) => {
                let ty: &FuncType = f.ty();
                format!(
                    "function ({:?})->({:?})",
                    ty.params().iter().map(|p| format!("{:?}", p)).join(", "),
                    ty.results().iter().map(|r| format!("{:?}", r)).join(", "),
                )
            }
            Extern::Global(g) => {
                let ty: GlobalType = g.ty();
                format!(
                    "global {}{:?}",
                    if ty.mutable { "mut " } else { "" },
                    ty.content_type
                )
            }
            Extern::Table(t) => {
                let ty: &TableType = t.ty();
                format!("table of {:?}", ty.element_type)
            }
            Extern::Memory(_m) => {
                format!("memory",)
            }
        }
    }
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

#[macro_export]
macro_rules! imports {
    (
        $(
            $module:literal : {
                $(
                    $name:literal : $ext:ident
                ),* $(,)?
            }
        ),* $(,)?
    ) => {
        vec![
            $(
                $(
                    $crate::wasp::externs::NamedExtern {
                        module: $module.to_string(),
                        name: $name.to_string(),
                        ext: $crate::wasp::externs::Extern::from($ext)
                    },
                )*
            )*
        ]
    };
}
