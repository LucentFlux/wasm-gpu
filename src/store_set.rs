//! This file outlines the concept of a single store_set 'instance', in the OOP sense, not the WASM sense,
//! created from the 'factory' of a StoreSet

pub mod builder;
pub mod ptrs;

use crate::instance::abstr::memory::MemoryPtr;
use crate::instance::concrete::global::GlobalInstanceSet;
use crate::instance::concrete::memory::MemoryInstanceSet;
use crate::instance::concrete::table::TableInstanceSet;
use crate::instance::data::DataInstance;
use crate::instance::element::ElementInstance;
use crate::instance::func::FuncsInstance;
use crate::memory::DynamicMemoryBlock;
use crate::Backend;
use anyhow::anyhow;
use std::hash::Hash;
use std::sync::Arc;

/// All of the state for a collection of active WASM state machines
/// Treated like an arena - everything allocated is allocated in here, and everyone else just keeps
/// 'references' in the form of usize indices into the FrozenVecs
pub struct StoreSet<B, T>
where
    B: Backend,
{
    backend: Arc<B>,
    data: Vec<T>,

    functions: Arc<FuncsInstance<B, T>>,
    elements: Arc<ElementInstance<B>>,
    datas: Arc<DataInstance<B>>,
    tables: TableInstanceSet<B>,
    memories: MemoryInstanceSet<B>,
    globals: GlobalInstanceSet<B>,
}

impl<B, T> StoreSet<B, T>
where
    B: Backend,
{
    pub fn data(&self, index: usize) -> Option<&T> {
        return self.data.get(index);
    }

    pub fn get_memory(
        &mut self,
        index: usize,
        ptr: MemoryPtr<B, T>,
    ) -> anyhow::Result<&mut DynamicMemoryBlock<B>> {
        self.memories
            .get_mut(ptr.get_ptr())
            .ok_or(anyhow!("memory pointer outside range"))
    }

    fn new(
        backend: Arc<B>,
        data: T,
        functions: Arc<FuncsInstance<B, T>>,
        elements: Arc<ElementInstance<B>>,
        datas: Arc<DataInstance<B>>,
        tables: TableInstanceSet<B>,
        memories: MemoryInstanceSet<B>,
        globals: GlobalInstanceSet<B>,
    ) -> Self {
        Self {
            backend,
            data,
            functions,
            tables,
            memories,
            globals,
            elements,
            datas,
        }
    }

    pub(crate) fn backend(&self) -> Arc<B> {
        self.backend.clone()
    }
}
