use crate::{Backend, FuncPtr};

pub enum Extern<B, T>
where
    B: Backend,
{
    Func(FuncPtr<B, T>),
    Global(Global),
    Table(Table),
    Memory(Memory),
    SharedMemory(SharedMemory),
}

impl<B, T> From<FuncPtr<B, T>> for Extern<B, T>
where
    B: Backend,
{
    fn from(f: FuncPtr<B, T>) -> Self {
        Self::Func(f)
    }
}

pub struct Global {}

pub struct Table {}

pub struct Memory {}

pub struct SharedMemory {}
