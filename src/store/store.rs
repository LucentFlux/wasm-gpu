//! This file outlines the concept of a single store 'instance', in the OOP sense, not the WASM sense,
//! created from the 'factory' of a StoreSet

use crate::externs::NamedExtern;
use crate::global_instance::GlobalInstance;
use crate::memory::DynamicMemoryBlock;
use crate::read_only::{AppendOnlyVec, ReadOnly};
use crate::store::ptrs::{FuncPtr, MemoryPtr, StorePtr};
use crate::{Backend, Func, Module, ModuleInstance};
use anyhow::anyhow;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// All of the state for one WASM state machine
/// Treated like an arena - everything allocated is allocated in here, and everyone else just keeps
/// 'references' in the form of usize indices into the FrozenVecs
pub struct Store<B, T>
where
    B: Backend,
{
    backend: Arc<B>,
    data: T,

    functions: Arc<AppendOnlyVec<Func<B, T>>>,
    tables: Vec<()>,
    memories: Vec<DynamicMemoryBlock<B>>,
    globals: GlobalInstance<B>,
    elements: Arc<AppendOnlyVec<()>>,
    datas: Arc<AppendOnlyVec<DynamicMemoryBlock<B>>>,

    parent_id: usize, // The ID of the StoreSet that this is a child of - ensures that pointers are valid
}

impl<B, T> Store<B, T>
where
    B: Backend,
{
    pub fn data(&self) -> &T {
        return &self.data;
    }

    pub fn get_memory(
        &mut self,
        ptr: MemoryPtr<B, T>,
    ) -> anyhow::Result<&mut DynamicMemoryBlock<B>> {
        assert_eq!(
            self.parent_id,
            ptr.get_store_id(),
            "memory pointer references different store"
        );

        self.memories
            .get_mut(ptr.get_ptr())
            .ok_or(anyhow!("memory pointer outside range"))
    }

    fn new(
        backend: Arc<B>,
        data: T,
        parent_id: usize,
        functions: Arc<AppendOnlyVec<Func<B, T>>>,
        tables: Vec<()>,
        memories: Vec<DynamicMemoryBlock<B>>,
        globals: GlobalInstance<B>,
        elements: Arc<AppendOnlyVec<()>>,
        datas: Arc<AppendOnlyVec<DynamicMemoryBlock<B>>>,
    ) -> Self {
        Self {
            parent_id,
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
}

impl<B, T> Hash for Store<B, T>
where
    B: Backend,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.id)
    }
}

impl<B, T> PartialEq<Self> for Store<B, T>
where
    B: Backend,
{
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<B, T> Eq for Store<B, T> where B: Backend {}

impl<B, T> PartialOrd<Self> for Store<B, T>
where
    B: Backend,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<B, T> Ord for Store<B, T>
where
    B: Backend,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}
