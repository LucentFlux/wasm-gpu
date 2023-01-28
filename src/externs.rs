use crate::instance::func::{TypedFuncPtr, UntypedFuncPtr};
use crate::instance::global::builder::AbstractGlobalPtr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use crate::instance::table::builder::AbstractTablePtr;
use crate::typed::WasmTyVec;
use itertools::Itertools;
use perfect_derive::perfect_derive;
use wasmparser::{FuncType, GlobalType, TableType};

#[perfect_derive(Clone)]
pub struct NamedExtern<T> {
    pub module: String,
    pub name: String,
    pub ext: Extern<T>,
}

#[derive(Debug)]
pub enum Extern<T> {
    Func(UntypedFuncPtr<T>),
    Global(AbstractGlobalPtr),
    Table(AbstractTablePtr),
    Memory(AbstractMemoryPtr),
}

impl<T> Extern<T> {
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

impl<T> Clone for Extern<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Func(f) => Self::Func(f.clone()),
            Self::Global(g) => Self::Global(g.clone()),
            Self::Table(t) => Self::Table(t.clone()),
            Self::Memory(m) => Self::Memory(m.clone()),
        }
    }
}

impl<T> From<UntypedFuncPtr<T>> for Extern<T> {
    fn from(f: UntypedFuncPtr<T>) -> Self {
        Self::Func(f)
    }
}

impl<T, Params, Results> From<TypedFuncPtr<T, Params, Results>> for Extern<T>
where
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    fn from(f: TypedFuncPtr<T, Params, Results>) -> Self {
        Self::Func(f.as_untyped().clone())
    }
}

impl<T> From<AbstractMemoryPtr> for Extern<T> {
    fn from(m: AbstractMemoryPtr) -> Self {
        Self::Memory(m)
    }
}

impl<T> From<AbstractGlobalPtr> for Extern<T> {
    fn from(g: AbstractGlobalPtr) -> Self {
        Self::Global(g)
    }
}

impl<T> From<AbstractTablePtr> for Extern<T> {
    fn from(t: AbstractTablePtr) -> Self {
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
