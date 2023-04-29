use crate::instance::func::{TypedFuncPtr, UntypedFuncPtr};
use crate::instance::global::builder::AbstractGlobalPtr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use crate::instance::table::builder::AbstractTablePtr;
use itertools::Itertools;
use perfect_derive::perfect_derive;
use wasm_types::WasmTyVec;
use wasmparser::{FuncType, GlobalType, TableType};

#[perfect_derive(Clone)]
pub struct NamedExtern {
    pub module: String,
    pub name: String,
    pub ext: Extern,
}

#[derive(Debug)]
pub enum Extern {
    Func(UntypedFuncPtr),
    Global(AbstractGlobalPtr),
    Table(AbstractTablePtr),
    Memory(AbstractMemoryPtr),
}

impl Extern {
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

impl Clone for Extern {
    fn clone(&self) -> Self {
        match self {
            Self::Func(f) => Self::Func(f.clone()),
            Self::Global(g) => Self::Global(g.clone()),
            Self::Table(t) => Self::Table(t.clone()),
            Self::Memory(m) => Self::Memory(m.clone()),
        }
    }
}

impl From<UntypedFuncPtr> for Extern {
    fn from(f: UntypedFuncPtr) -> Self {
        Self::Func(f)
    }
}

impl<Params, Results> From<TypedFuncPtr<Params, Results>> for Extern
where
    Params: WasmTyVec,
    Results: WasmTyVec,
{
    fn from(f: TypedFuncPtr<Params, Results>) -> Self {
        Self::Func(f.as_untyped().clone())
    }
}

impl From<AbstractMemoryPtr> for Extern {
    fn from(m: AbstractMemoryPtr) -> Self {
        Self::Memory(m)
    }
}

impl From<AbstractGlobalPtr> for Extern {
    fn from(g: AbstractGlobalPtr) -> Self {
        Self::Global(g)
    }
}

impl From<AbstractTablePtr> for Extern {
    fn from(t: AbstractTablePtr) -> Self {
        Self::Table(t)
    }
}
