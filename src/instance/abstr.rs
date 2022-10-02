//! When building store sets, we first move all of the data to the GPU, then we duplicate it as
//! required to create multiple 'concrete' wasm state machines. We call this intermediate step,
//! before duplication, 'abstract instantiation'

pub mod global;
pub mod memory;
pub mod table;
