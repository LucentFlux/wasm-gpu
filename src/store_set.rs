pub mod builder;
pub mod calling;

use wgpu_lazybuffers_macros::lazy_mappable;

use crate::func::assembled_module::AssembledModule;
use crate::instance::data::UnmappedDataInstance;
use crate::instance::element::UnmappedElementInstance;
use crate::instance::func::FuncsInstance;
use crate::instance::global::immutable::UnmappedImmutableGlobalsInstance;
use crate::instance::global::instance::{
    MappedMutableGlobalsInstanceSet, UnmappedMutableGlobalsInstanceSet,
};
use crate::instance::memory::instance::{MappedMemoryInstanceSet, UnmappedMemoryInstanceSet};
use crate::instance::table::instance::{MappedTableInstanceSet, UnmappedTableInstanceSet};
use crate::MappedStoreSetBuilder;
use std::sync::Arc;

#[lazy_mappable(MappedStoreSetData)]
pub struct UnmappedStoreSetData {
    #[map(MappedTableInstanceSet)]
    pub tables: UnmappedTableInstanceSet,
    #[map(MappedMemoryInstanceSet)]
    pub memories: UnmappedMemoryInstanceSet,
    #[map(MappedMutableGlobalsInstanceSet)]
    pub mutable_globals: UnmappedMutableGlobalsInstanceSet,
}

/// All of the state for a collection of active WASM state machines
pub struct StoreSet<T, O> {
    pub data: Vec<T>,

    pub functions: Arc<FuncsInstance<T>>,
    pub elements: Arc<UnmappedElementInstance>,
    pub datas: Arc<UnmappedDataInstance>,
    pub immutable_globals: Arc<UnmappedImmutableGlobalsInstance>,

    pub assembled_module: Arc<AssembledModule>,

    pub owned: O,
}

pub type DeviceStoreSet<T> = StoreSet<T, UnmappedStoreSetData>;
pub type HostStoreSet<T> = StoreSet<T, MappedStoreSetData>;

impl<T> DeviceStoreSet<T> {
    /// Use current module state to form a new store set builder, with all values initialised to the
    /// parts contained in this. This is similar to the [Wizer](https://github.com/bytecodealliance/wizer)
    /// project, except the idea of snapshots is more important here since this is the mechanism
    /// used to invoke a function, then link a new module, then invoke again.
    ///
    /// Executing `builder.complete().await.build(vec![...]).await.seed_builder(0).await` is an
    /// identity on an object of type `StoreSetBuilder` and you will get back to where you started
    /// (with lots of unnecessary memory operations, and as long as the iterator passed into `build`
    /// isn't empty).
    pub async fn snapshot(&self, store_index: usize) -> MappedStoreSetBuilder<T> {
        MappedStoreSetBuilder::snapshot(&self, store_index).await
    }
}
