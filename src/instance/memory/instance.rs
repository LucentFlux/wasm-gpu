use std::ops::{Bound, Range, RangeBounds};

use crate::capabilities::CapabilityStore;
use crate::impl_concrete_ptr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use wasm_gpu_funcgen::MEMORY_STRIDE_WORDS;
use wgpu::BufferAsyncError;
use wgpu_async::{async_device::OutOfMemoryError, async_queue::AsyncQueue};
use wgpu_lazybuffers::{LockCollection, MemorySystem, UnmappedLazyBuffer};
use wgpu_lazybuffers_interleaving::{
    Interleaveable, InterleavedBufferConfig, MappedInterleavedBuffer, UnmappedInterleavedBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

pub const MEMORY_STRIDE_BYTES: u64 = (MEMORY_STRIDE_WORDS * 4) as u64;

#[lazy_mappable(MappedMemoryInstanceSet)]
pub struct UnmappedMemoryInstanceSet {
    #[map(MappedInterleavedBuffer<MEMORY_STRIDE_BYTES>)]
    memory: UnmappedInterleavedBuffer<MEMORY_STRIDE_BYTES>,
    cap_set: CapabilityStore,
    instance_count: usize,
}

impl UnmappedMemoryInstanceSet {
    pub(crate) async fn try_new(
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        source: &UnmappedLazyBuffer,
        instance_count: usize,
        cap_set: CapabilityStore,
    ) -> Result<Self, OutOfMemoryError> {
        let cfg = InterleavedBufferConfig {
            label: &format!("{}_instance_set", source.label()),
            repetitions: instance_count,
            usages: wgpu::BufferUsages::STORAGE,
            locking_size: None,
            transfer_size: None,
        };
        let memory = source
            .try_duplicate_interleave(memory_system, queue, &cfg)
            .await?;

        Ok(Self {
            memory,
            cap_set,
            instance_count,
        })
    }

    pub(crate) fn buffer(&self) -> &UnmappedLazyBuffer {
        &self.memory
    }
}

impl MappedMemoryInstanceSet {
    pub(crate) async fn lock<'a>(&'a self, ptr: &MemoryPtr) -> MemoryView<'a> {
        assert!(self.cap_set.check(&ptr.src.cap));

        let mut memory_locks = LockCollection::empty();
        let bounds = ptr.bounds(self.instance_count);
        self.memory
            .lock_writing(bounds.clone(), &mut memory_locks)
            .await;

        MemoryView {
            memory: &self.memory,
            memory_locks,
            bounds,
        }
    }
}

pub struct MemoryView<'a> {
    memory: &'a MappedInterleavedBuffer<MEMORY_STRIDE_BYTES>,
    memory_locks: LockCollection<'a>,
    bounds: Range<usize>,
}

impl<'a> MemoryView<'a> {
    fn slice_to_bounds(&self, slice: impl RangeBounds<usize>) -> Range<usize> {
        let start = self.bounds.start
            + match slice.start_bound() {
                Bound::Included(v) => *v,
                Bound::Excluded(v) => *v + 1,
                Bound::Unbounded => 0,
            };
        let end = match slice.end_bound() {
            Bound::Included(v) => self.bounds.start + *v + 1,
            Bound::Excluded(v) => self.bounds.start + *v,
            Bound::Unbounded => self.bounds.end,
        };

        assert!(start <= end);
        assert!(end <= self.bounds.end);

        return start..end;
    }

    pub async fn try_read_slice(
        &self,
        queue: &AsyncQueue,
        slice: impl RangeBounds<usize>,
    ) -> Result<Vec<u8>, BufferAsyncError> {
        self.memory
            .try_read_slice_with_locks(queue, self.slice_to_bounds(slice), &self.memory_locks)
            .await
    }

    pub async fn try_write_slice(
        &self,
        queue: &AsyncQueue,
        slice: impl RangeBounds<usize>,
        data: &[u8],
    ) -> Result<(), BufferAsyncError> {
        self.memory
            .try_write_slice_with_locks(
                queue,
                self.slice_to_bounds(slice),
                data,
                &self.memory_locks,
            )
            .await
    }
}

impl_concrete_ptr!(
    pub struct MemoryPtr {
        data...
    } with abstract AbstractMemoryPtr;
);

impl MemoryPtr {
    fn bounds(&self, instance_count: usize) -> Range<usize> {
        let segment_len = self.src.len();
        let start = (self.src.ptr * instance_count) + (self.index * segment_len);
        return start..(start + segment_len);
    }
}
