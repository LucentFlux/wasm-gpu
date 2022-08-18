use crate::func::Func;

pub enum Extern {
    Func(Func),
    Global(Global),
    Table(Table),
    Memory(Memory),
    SharedMemory(SharedMemory),
}

pub struct Global {}

pub struct Table {}

pub struct Memory {}

pub struct SharedMemory {}
