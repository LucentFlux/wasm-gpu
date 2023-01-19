use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::memory::instance::{MemoryPtr, UnmappedMemoryInstanceSet};
use wasmparser::MemoryType;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    DelayedOutOfMemoryError, EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem,
    UnmappedLazyBuffer,
};

use super::wasm_limits_match;

pub struct UnmappedMemoryInstanceSetBuilder {
    memories: Vec<UnmappedLazyBuffer>,
    cap_set: CapabilityStore,
}

impl UnmappedMemoryInstanceSetBuilder {
    pub async fn build(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        count: usize,
    ) -> UnmappedMemoryInstanceSet {
        UnmappedMemoryInstanceSet::new(
            memory_system,
            queue,
            &self.memories,
            count,
            self.cap_set.clone(),
        )
        .await
    }
}

pub struct MappedMemoryInstanceSetBuilder {
    memories: Vec<MappedLazyBuffer>,
    cap_set: CapabilityStore,
    memory_system: MemorySystem,
}

impl MappedMemoryInstanceSetBuilder {
    pub fn new(memory_system: &MemorySystem) -> Self {
        Self {
            cap_set: CapabilityStore::new(0),
            memories: Vec::new(),
            memory_system: memory_system.clone(),
        }
    }

    pub fn add_memory<T>(&mut self, plan: &MemoryType) -> AbstractMemoryPtr<T> {
        let ptr = self.memories.len();
        self.memories.push(
            self.memory_system
                .create_and_map_empty(&EmptyMemoryBlockConfig {
                    usages: wgpu::BufferUsages::STORAGE,
                    locking_size: None,
                }),
        );
        self.cap_set = self.cap_set.resize_ref(self.memories.len());
        return AbstractMemoryPtr::new(ptr, self.cap_set.get_cap(), plan.clone());
    }

    /// # Panics
    /// Panics if the pointer is not for this abstract memory
    pub async fn initialize<T>(&mut self, ptr: &AbstractMemoryPtr<T>, data: &[u8], offset: usize) {
        assert!(
            self.cap_set.check(&ptr.cap),
            "memory pointer was not valid for this instance"
        );

        self.memories
            .get_mut(ptr.ptr as usize)
            .expect("Memory builders are append only, so having a pointer implies the item exists")
            .write(data, offset)
            .await
    }

    pub async fn unmap(
        self,
        queue: &AsyncQueue,
    ) -> Result<UnmappedMemoryInstanceSetBuilder, DelayedOutOfMemoryError<Self>> {
        let memories = self
            .memories
            .unmap_all(queue)
            .map_oom(|memories| Self { memories, ..self })?;

        return Ok(UnmappedMemoryInstanceSetBuilder {
            cap_set: self.cap_set,
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
