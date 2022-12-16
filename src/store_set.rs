use crate::instance::data::UnmappedDataInstance;
use crate::instance::element::UnmappedElementInstance;
use crate::instance::func::FuncsInstance;
use crate::instance::global::immutable::UnmappedImmutableGlobalsInstance;
use crate::instance::global::instance::{
    MappedMutableGlobalInstanceSet, UnmappedMutableGlobalInstanceSet,
};
use crate::instance::memory::instance::{MappedMemoryInstanceSet, UnmappedMemoryInstanceSet};
use crate::instance::table::instance::{MappedTableInstanceSet, UnmappedTableInstanceSet};
use crate::StoreSetBuilder;
use lf_hal::backend::Backend;
use std::sync::Arc;

pub mod builder;

pub struct DeviceStoreSetData<B>
where
    B: Backend,
{
    pub tables: UnmappedTableInstanceSet<B>,
    pub memories: UnmappedMemoryInstanceSet<B>,
    pub mutable_globals: UnmappedMutableGlobalInstanceSet<B>,
}

pub struct HostStoreSetData<B>
where
    B: Backend,
{
    pub tables: MappedTableInstanceSet<B>,
    pub memories: MappedMemoryInstanceSet<B>,
    pub mutable_globals: MappedMutableGlobalInstanceSet<B>,
}

/// All of the state for a collection of active WASM state machines
pub struct StoreSet<B, T, O>
where
    B: Backend,
{
    pub backend: Arc<B>,
    pub data: Vec<T>,

    pub functions: Arc<FuncsInstance<B, T>>,
    pub elements: Arc<UnmappedElementInstance<B>>,
    pub datas: Arc<UnmappedDataInstance<B>>,
    pub immutable_globals: Arc<UnmappedImmutableGlobalsInstance<B>>,
    pub owned: O,
}

pub type DeviceStoreSet<B, T> = StoreSet<B, T, DeviceStoreSetData<B>>;
pub type HostStoreSet<B, T> = StoreSet<B, T, HostStoreSetData<B>>;

impl<B: Backend, T> DeviceStoreSet<B, T> {
    /// Use current module state to form a new store set builder, with all values initialised to the
    /// parts contained in this. This is similar to the [Wizer](https://github.com/bytecodealliance/wizer)
    /// project, except the idea of snapshots is more important here since this is the mechanism
    /// used to invoke a function, then link a new module, then invoke again.
    ///
    /// Executing `builder.complete().await.build(vec![...]).await.seed_builder(0).await` is an
    /// identity on an object of type `StoreSetBuilder` and you will get back to where you started
    /// (with lots of unnecessary memory operations, and as long as the iterator passed into `build`
    /// isn't empty).
    pub async fn snapshot(&self, store_index: usize) -> StoreSetBuilder<B, T> {
        StoreSetBuilder::snapshot(&self, store_index).await
    }
}
