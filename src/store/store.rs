//! This file outlines the concept of a single store 'instance', in the OOP sense, not the WASM sense,
//! created from the 'factory' of a StoreSet

use crate::instance::abstr::memory::MemoryPtr;
use crate::memory::DynamicMemoryBlock;
use crate::read_only::AppendOnlyVec;
use crate::{Backend, Func};
use anyhow::anyhow;
use std::cmp::Ordering;
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

    // Data to ensure that pointers are valid at runtime
    abstract_id: usize, // The ID of the abstract StoreSet that this is a child of
    concrete_id: usize, // The ID of this store specifically
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
            self.abstract_id, ptr.abstract_id,
            "memory pointer references different abstract store"
        );

        assert_eq!(
            self.concrete_id, ptr.concrete_id,
            "memory pointer references different concrete store"
        );

        self.memories
            .get_mut(ptr.get_ptr())
            .ok_or(anyhow!("memory pointer outside range"))
    }

    fn new(
        backend: Arc<B>,
        data: T,
        abstract_id: usize,
        concrete_id: usize,
        functions: Arc<AppendOnlyVec<Func<B, T>>>,
        tables: Vec<()>,
        memories: Vec<DynamicMemoryBlock<B>>,
        globals: GlobalInstance<B>,
        elements: Arc<AppendOnlyVec<()>>,
        datas: Arc<AppendOnlyVec<DynamicMemoryBlock<B>>>,
    ) -> Self {
        Self {
            abstract_id,
            concrete_id,
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

    pub(crate) fn get_concrete_id(&self) -> usize {
        self.concrete_id
    }

    pub(crate) fn backend(&self) -> Arc<B> {
        self.backend.clone()
    }
}

impl<B, T> Hash for Store<B, T>
where
    B: Backend,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.abstract_id);
        state.write_usize(self.concrete_id);
    }
}

impl<B, T> PartialEq<Self> for Store<B, T>
where
    B: Backend,
{
    fn eq(&self, other: &Self) -> bool {
        self.abstract_id == other.abstract_id && self.concrete_id == other.concrete_id
    }
}

impl<B, T> Eq for Store<B, T> where B: Backend {}

impl<B, T> PartialOrd<Self> for Store<B, T>
where
    B: Backend,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<B, T> Ord for Store<B, T>
where
    B: Backend,
{
    fn cmp(&self, other: &Self) -> Ordering {
        match self.abstract_id.cmp(&other.abstract_id) {
            Ordering::Equal => self.concrete_id.cmp(&other.concrete_id),
            v => v,
        }
    }
}
