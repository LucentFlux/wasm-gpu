use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::memory::instance::{MemoryPtr, UnmappedMemoryInstanceSet};
use wasmparser::MemoryType;
use wgpu::BufferAsyncError;
use wgpu_async::async_device::OutOfMemoryError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::DelayedOutOfMemoryResult;
use wgpu_lazybuffers::MappedLazyBufferIter;
use wgpu_lazybuffers::{
    DelayedOutOfMemoryError, EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem,
    UnmappedLazyBuffer,
};

use super::wasm_limits_match;

#[derive(Debug, Clone)]
struct Meta {
    cap_set: CapabilityStore,
    memory_system: MemorySystem,
}

#[derive(Debug)]
pub struct UnmappedMemoryInstanceSetBuilder {
    memories: Vec<UnmappedLazyBuffer>,
    meta: Meta,
}

impl UnmappedMemoryInstanceSetBuilder {
    pub async fn build(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        count: usize,
    ) -> Result<UnmappedMemoryInstanceSet, OutOfMemoryError> {
        UnmappedMemoryInstanceSet::new(
            memory_system,
            queue,
            &self.memories,
            count,
            self.meta.cap_set.clone(),
        )
        .await
    }

    pub fn map(self) -> MappedMemoryInstanceSetBuilder {
        let Self { memories, meta } = self;

        MappedMemoryInstanceSetBuilder {
            memories: memories.into_iter().map(UnmappedLazyBuffer::map).collect(),
            meta,
        }
    }
}

#[derive(Debug)]
pub struct MappedMemoryInstanceSetBuilder {
    memories: Vec<MappedLazyBuffer>,
    meta: Meta,
}

impl MappedMemoryInstanceSetBuilder {
    pub fn new(memory_system: &MemorySystem) -> Self {
        Self {
            memories: Vec::new(),
            meta: Meta {
                cap_set: CapabilityStore::new(0),
                memory_system: memory_system.clone(),
            },
        }
    }

    pub fn add_memory<T>(&mut self, plan: &MemoryType) -> AbstractMemoryPtr<T> {
        let ptr = self.memories.len();
        self.memories.push(
            self.meta
                .memory_system
                .create_and_map_empty(&EmptyMemoryBlockConfig {
                    usages: wgpu::BufferUsages::empty(),
                    locking_size: 8192,
                }),
        );
        self.meta.cap_set = self.meta.cap_set.resize_ref(self.memories.len());
        return AbstractMemoryPtr::new(ptr, self.meta.cap_set.get_cap(), plan.clone());
    }

    /// # Panics
    /// Panics if the pointer is not for this abstract memory
    pub async fn initialize<T>(
        &mut self,
        queue: &AsyncQueue,
        ptr: &AbstractMemoryPtr<T>,
        data: &[u8],
        offset: usize,
    ) -> Result<(), BufferAsyncError> {
        assert!(
            self.meta.cap_set.check(&ptr.cap),
            "memory pointer was not valid for this instance"
        );

        self.memories
            .get_mut(ptr.ptr as usize)
            .expect("Memory builders are append only, so having a pointer implies the item exists")
            .write_slice(queue, offset..offset + data.len(), data)
            .await
    }

    pub async fn unmap(
        self,
        queue: &AsyncQueue,
    ) -> Result<UnmappedMemoryInstanceSetBuilder, DelayedOutOfMemoryError<Self>> {
        let memories = self
            .memories
            .unmap_all(queue)
            .await
            .map_oom(|memories| Self {
                memories,
                meta: self.meta.clone(),
                ..self
            })?;

        return Ok(UnmappedMemoryInstanceSetBuilder {
            meta: self.meta,
            memories,
        });
    }
}

impl_abstract_ptr!(
    pub struct AbstractMemoryPtr<T> {
        pub(in crate::instance::memory) data...
        // Copied from Memory
        ty: MemoryType,
    } with concrete MemoryPtr<T>;
);

impl<T> AbstractMemoryPtr<T> {
    pub fn is_type(&self, ty: &MemoryType) -> bool {
        wasm_limits_match(self.ty.initial, self.ty.maximum, ty.initial, ty.maximum)
    }
}
