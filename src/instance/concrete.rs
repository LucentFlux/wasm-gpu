//! When building store_set sets, we first move all of the data to the GPU, then we duplicate it as
//! required to create multiple 'concrete' wasm state machines. The files in this namespace should
//! mirror crate::instance::abstr as they should be concretized versions of the abstract builder step.
//! Constructing these concrete versions of the abstract types should also transfer little to no data
//! to the GPU - the point of the abstract versions is to transfer once and then build on the GPU.

pub mod global;
pub mod memory;
pub mod table;
