//! This file outlines the concept of a single store_set 'instance', in the OOP sense, not the WASM sense,
//! created from the 'factory' of a StoreSet

use crate::instance::data::DeviceDataInstance;
use crate::instance::element::DeviceElementInstance;
use crate::instance::func::FuncsInstance;
use crate::instance::global::concrete::{
    DeviceMutableGlobalInstanceSet, HostMutableGlobalInstanceSet,
};
use crate::instance::global::immutable::DeviceImmutableGlobalsInstance;
use crate::instance::memory::concrete::{DeviceMemoryInstanceSet, HostMemoryInstanceSet};
use crate::instance::table::concrete::{DeviceTableInstanceSet, HostTableInstanceSet};
use crate::Backend;
use std::sync::Arc;

pub mod builder;

pub struct DeviceStoreSetData<B>
where
    B: Backend,
{
    pub tables: DeviceTableInstanceSet<B>,
    pub memories: DeviceMemoryInstanceSet<B>,
    pub mutable_globals: DeviceMutableGlobalInstanceSet<B>,
}

pub struct HostStoreSetData<B>
where
    B: Backend,
{
    pub tables: HostTableInstanceSet<B>,
    pub memories: HostMemoryInstanceSet<B>,
    pub mutable_globals: HostMutableGlobalInstanceSet<B>,
}

/// All of the state for a collection of active WASM state machines
pub struct StoreSet<B, T, O>
where
    B: Backend,
{
    pub backend: Arc<B>,
    pub data: Vec<T>,

    pub functions: Arc<FuncsInstance<B, T>>,
    pub elements: Arc<DeviceElementInstance<B>>,
    pub datas: Arc<DeviceDataInstance<B>>,
    pub immutable_globals: Arc<DeviceImmutableGlobalsInstance<B>>,
    pub owned: O,
}

pub type DeviceStoreSet<B: Backend, T> = StoreSet<B, T, DeviceStoreSetData<B>>;
pub type HostStoreSet<B: Backend, T> = StoreSet<B, T, HostStoreSetData<B>>;
