//! This file outlines the concept of a single store_set 'instance', in the OOP sense, not the WASM sense,
//! created from the 'factory' of a StoreSet

use crate::instance::data::DeviceDataInstance;
use crate::instance::element::DeviceElementInstance;
use crate::instance::func::FuncsInstance;
use crate::instance::global::concrete::DeviceGlobalInstanceSet;
use crate::instance::memory::concrete::DeviceMemoryInstanceSet;
use crate::instance::table::concrete::DeviceTableInstanceSet;
use crate::Backend;
use std::sync::Arc;

pub mod builder;

/// All of the state for a collection of active WASM state machines
pub struct StoreSet<B, T>
where
    B: Backend,
{
    pub backend: Arc<B>,
    pub data: Vec<T>,

    pub functions: Arc<FuncsInstance<B, T>>,
    pub elements: Arc<DeviceElementInstance<B>>,
    pub datas: Arc<DeviceDataInstance<B>>,
    pub tables: DeviceTableInstanceSet<B>,
    pub memories: DeviceMemoryInstanceSet<B>,
    pub globals: DeviceGlobalInstanceSet<B>,
}
