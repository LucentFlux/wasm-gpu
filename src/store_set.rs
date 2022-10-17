//! This file outlines the concept of a single store_set 'instance', in the OOP sense, not the WASM sense,
//! created from the 'factory' of a StoreSet

use crate::instance::data::DataInstance;
use crate::instance::element::ElementInstance;
use crate::instance::func::FuncsInstance;
use crate::instance::global::concrete::GlobalInstanceSet;
use crate::instance::memory::concrete::MemoryPtr;
use crate::instance::memory::concrete::{MemoryInstanceSet, MemoryInstanceView};
use crate::instance::table::concrete::TableInstanceSet;
use crate::Backend;
use std::sync::Arc;

pub mod builder;

/// All of the state for a collection of active WASM state machines
/// Treated like an arena - everything allocated is allocated in here, and everyone else just keeps
/// 'references' in the form of usize indices into the FrozenVecs
pub struct StoreSet<B, T>
where
    B: Backend,
{
    pub backend: Arc<B>,
    pub data: Vec<T>,

    pub functions: Arc<FuncsInstance<B, T>>,
    pub elements: Arc<ElementInstance<B>>,
    pub datas: Arc<DataInstance<B>>,
    pub tables: TableInstanceSet<B>,
    pub memories: MemoryInstanceSet<B>,
    pub globals: GlobalInstanceSet<B>,
}
