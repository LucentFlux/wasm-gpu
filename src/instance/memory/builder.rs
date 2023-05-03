use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::memory::instance::{MemoryPtr, UnmappedMemoryInstanceSet};
use wasmparser::MemoryType;
use wasmtime_environ::WASM_PAGE_SIZE;
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
    #[map(MappedLazyBuffer)]
    memory: UnmappedLazyBuffer,
    cap_set: CapabilityStore,
}

impl UnmappedMemoryInstanceSetBuilder {
    pub async fn try_build(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        count: usize,
    ) -> Result<UnmappedMemoryInstanceSet, OutOfMemoryError> {
        UnmappedMemoryInstanceSet::try_new(
            memory_system,
            queue,
            &self.memory,
            count,
            self.cap_set.clone(),
        )
        .await
    }
}

impl MappedMemoryInstanceSetBuilder {
    pub fn new(memory_system: &MemorySystem, module_label: &str) -> Self {
        Self {
            memory: memory_system.create_and_map_empty(&EmptyMemoryBlockConfig {
                label: &format!("{}_memory_buffer", module_label),
                usages: wgpu::BufferUsages::empty(),
                locking_size: 8192,
                transfer_size: 4096,
            }),
            cap_set: CapabilityStore::new(0),
        }
    }

    pub fn add_memory(&mut self, plan: &MemoryType) -> AbstractMemoryPtr {
        let ptr = self.memory.len();
        let len = usize::try_from(plan.initial * WASM_PAGE_SIZE as u64)
            .expect("memory must be expressable in RAM, but was too big");
        self.memory.extend_lazy(len);
        self.cap_set = self.cap_set.resize_ref(self.memory.len());
        return AbstractMemoryPtr::new(ptr, self.cap_set.get_cap(), plan.clone(), len);
    }

    /// # Panics
    /// Panics if the pointer is not for this abstract memory
    pub async fn try_initialize(
        &mut self,
        queue: &AsyncQueue,
        ptr: &AbstractMemoryPtr,
        data: &[u8],
        offset: usize,
    ) -> Result<(), BufferAsyncError> {
        assert!(
            self.cap_set.check(&ptr.cap),
            "memory pointer was not valid for this instance"
        );

        assert!(
            ptr.len >= offset + data.len(),
            "cannot slice memory larger than allocated memory space"
        );

        let bounds = (ptr.ptr + offset)..(ptr.ptr + offset + data.len());

        self.memory
            .try_write_slice_locking(queue, bounds, data)
            .await
    }
}

impl_abstract_ptr!(
    pub struct AbstractMemoryPtr {
        pub(in crate::instance::memory) data...
        // Copied from Memory
        ty: MemoryType,
        len: usize,
    } with concrete MemoryPtr;
);

impl AbstractMemoryPtr {
    pub fn is_type(&self, ty: &MemoryType) -> bool {
        wasm_limits_match(self.ty.initial, self.ty.maximum, ty.initial, ty.maximum)
    }

    pub fn to_index(&self) -> wasm_gpu_funcgen::MemoryIndex {
        wasm_gpu_funcgen::MemoryIndex::from(self.ptr)
    }
}
