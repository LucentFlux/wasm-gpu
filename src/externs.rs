use crate::instance::global::GlobalPtr;
use crate::store::ptrs::{FuncPtr, MemoryPtr, StorePtr, TablePtr};
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
    Func(FuncPtr<B, T>),
    Global(GlobalPtr<B, T>),
    Table(TablePtr<B, T>),
    Memory(MemoryPtr<B, T>),
}

impl<B, T> StorePtr for Extern<B, T>
where
    B: Backend,
{
    fn get_store_id(&self) -> usize {
        match self {
            Extern::Func(sp) | Extern::Global(sp) | Extern::Table(sp) | Extern::Memory(sp) => {
                sp.get_store_id()
            }
        }
    }

    fn get_ptr(&self) -> usize {
        match self {
            Extern::Func(sp) | Extern::Global(sp) | Extern::Table(sp) | Extern::Memory(sp) => {
                sp.get_ptr()
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

impl<B, T> From<FuncPtr<B, T>> for Extern<B, T>
where
    B: Backend,
{
    fn from(f: FuncPtr<B, T>) -> Self {
        Self::Func(f)
    }
}

impl<B, T> From<MemoryPtr<B, T>> for Extern<B, T>
where
    B: Backend,
{
    fn from(m: MemoryPtr<B, T>) -> Self {
        Self::Memory(m)
    }
}

impl<B, T> From<GlobalPtr<B, T>> for Extern<B, T>
where
    B: Backend,
{
    fn from(g: GlobalPtr<B, T>) -> Self {
        Self::Global(g)
    }
}

impl<B, T> From<TablePtr<B, T>> for Extern<B, T>
where
    B: Backend,
{
    fn from(t: TablePtr<B, T>) -> Self {
        Self::Table(t)
    }
}

impl<V, B, T> From<&V> for Extern<B, T>
where
    V: Clone,
    Extern<B, T>: From<V>,
    B: Backend,
{
    fn from(v: &V) -> Self {
        Self::from(v.clone())
    }
}
