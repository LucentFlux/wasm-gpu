use std::ops::RangeBounds;

use crate::capabilities::CapabilityStore;
use crate::impl_concrete_ptr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use futures::future::join_all;
use wasm_spirv_funcgen::MEMORY_STRIDE_WORDS;
use wgpu::BufferAsyncError;
use wgpu_async::{async_device::OutOfMemoryError, async_queue::AsyncQueue};
use wgpu_lazybuffers::{MemorySystem, UnmappedLazyBuffer};
use wgpu_lazybuffers_interleaving::{
    Interleaveable, InterleavedBufferConfig, MappedInterleavedBuffer, UnmappedInterleavedBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

pub const MEMORY_STRIDE_BYTES: u64 = (MEMORY_STRIDE_WORDS * 4) as u64;

#[lazy_mappable(MappedMemoryInstanceSet)]
pub struct UnmappedMemoryInstanceSet {
    #[map(Vec<MappedInterleavedBuffer<MEMORY_STRIDE_BYTES>>)]
    data: Vec<UnmappedInterleavedBuffer<MEMORY_STRIDE_BYTES>>,
    cap_set: CapabilityStore,
}

impl UnmappedMemoryInstanceSet {
    pub(crate) async fn try_new(
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        sources: &Vec<UnmappedLazyBuffer>,
        count: usize,
        cap_set: CapabilityStore,
    ) -> Result<Self, OutOfMemoryError> {
        let cfg = InterleavedBufferConfig {
            repetitions: count,
            usages: wgpu::BufferUsages::STORAGE,
            locking_size: None,
        };
        let memories = sources
            .iter()
            .map(|source| source.try_duplicate_interleave(memory_system, queue, &cfg));

        let memories: Result<_, _> = join_all(memories).await.into_iter().collect();

        Ok(Self {
            data: memories?,
            cap_set,
        })
    }
}

impl MappedMemoryInstanceSet {
    pub(crate) fn get<'a>(&'a self, ptr: &MemoryPtr) -> MemoryView<'a> {
        assert!(self.cap_set.check(&ptr.src.cap));

        let memory_block = self
            .data
            .get(ptr.src.ptr)
            .expect("pointer was valid in cap set");

        MemoryView {
            memory_block,
            index: ptr.index,
        }
    }
}

pub struct MemoryView<'a> {
    memory_block: &'a MappedInterleavedBuffer<MEMORY_STRIDE_BYTES>,
    index: usize,
}

impl<'a> MemoryView<'a> {
    pub async fn try_read_slice(
        &self,
        queue: &AsyncQueue,
        slice: impl RangeBounds<usize>,
    ) -> Result<Vec<u8>, BufferAsyncError> {
        self.memory_block
            .try_read_interleaved_slice_locking(queue, slice, self.index)
            .await
    }

    pub async fn try_write_slice(
        &self,
        queue: &AsyncQueue,
        slice: impl RangeBounds<usize>,
        data: &[u8],
    ) -> Result<(), BufferAsyncError> {
        self.memory_block
            .try_write_interleaved_slice_locking(queue, slice, self.index, data)
            .await
    }
}

impl_concrete_ptr!(
    pub struct MemoryPtr {
        data...
    } with abstract AbstractMemoryPtr;
);
