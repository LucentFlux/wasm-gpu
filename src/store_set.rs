//! This file outlines the concept of a single store_set 'instance', in the OOP sense, not the WASM sense,
//! created from the 'factory' of a StoreSet

use crate::instance::data::UnmappedDataInstance;
use crate::instance::element::UnmappedElementInstance;
use crate::instance::func::FuncsInstance;
use crate::instance::global::immutable::DeviceImmutableGlobalsInstance;
use crate::instance::global::instance::{
    MappedMutableGlobalInstanceSet, UnmappedMutableGlobalInstanceSet,
};
use crate::instance::memory::instance::{MappedMemoryInstanceSet, UnmappedMemoryInstanceSet};
use crate::instance::table::instance::{MappedTableInstanceSet, UnmappedTableInstanceSet};
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
    pub immutable_globals: Arc<DeviceImmutableGlobalsInstance<B>>,
    pub owned: O,
}

pub type DeviceStoreSet<B, T> = StoreSet<B, T, DeviceStoreSetData<B>>;
pub type HostStoreSet<B, T> = StoreSet<B, T, HostStoreSetData<B>>;
