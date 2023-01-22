use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::memory::instance::{MemoryPtr, UnmappedMemoryInstanceSet};
use wasmparser::MemoryType;
use wgpu::BufferAsyncError;
use wgpu_async::async_device::OutOfMemoryError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem, UnmappedLazyBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

use super::wasm_limits_match;

#[derive(Debug, Clone)]
struct Meta {}

#[lazy_mappable(MappedMemoryInstanceSetBuilder)]
#[derive(Debug)]
pub struct UnmappedMemoryInstanceSetBuilder {
    #[map(Vec<MappedLazyBuffer>)]
    memories: Vec<UnmappedLazyBuffer>,
    cap_set: CapabilityStore,
    memory_system: MemorySystem,
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
            self.cap_set.clone(),
        )
        .await
    }
}

impl MappedMemoryInstanceSetBuilder {
    pub fn new(memory_system: &MemorySystem) -> Self {
        Self {
            memories: Vec::new(),
            cap_set: CapabilityStore::new(0),
            memory_system: memory_system.clone(),
        }
    }

    pub fn add_memory<T>(&mut self, plan: &MemoryType) -> AbstractMemoryPtr<T> {
        let ptr = self.memories.len();
        self.memories.push(
            self.memory_system
                .create_and_map_empty(&EmptyMemoryBlockConfig {
                    usages: wgpu::BufferUsages::empty(),
                    locking_size: 8192,
                }),
        );
        self.cap_set = self.cap_set.resize_ref(self.memories.len());
        return AbstractMemoryPtr::new(ptr, self.cap_set.get_cap(), plan.clone());
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
            self.cap_set.check(&ptr.cap),
            "memory pointer was not valid for this instance"
        );

        self.memories
            .get_mut(ptr.ptr as usize)
            .expect("Memory builders are append only, so having a pointer implies the item exists")
            .write_slice(queue, offset..offset + data.len(), data)
            .await
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
