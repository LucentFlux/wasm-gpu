use crate::capabilities::CapabilityStore;
use crate::impl_concrete_ptr;
use crate::instance::global::builder::AbstractGlobalMutablePtr;
use wgpu_async::{async_device::OutOfMemoryError, async_queue::AsyncQueue};
use wgpu_lazybuffers::{MemorySystem, UnmappedLazyBuffer};
use wgpu_lazybuffers_interleaving::{
    Interleaveable, InterleavedBufferConfig, MappedInterleavedBuffer, UnmappedInterleavedBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

const STRIDE: u64 = 4; // 1 * u32

#[lazy_mappable(MappedMutableGlobalsInstanceSet)]
pub struct UnmappedMutableGlobalsInstanceSet {
    #[map(MappedInterleavedBuffer<STRIDE>)]
    mutables: UnmappedInterleavedBuffer<STRIDE>,

    cap_set: CapabilityStore,

    /// Number of bytes used in buffer. Used when reconstructing builder
    head: usize,
}

impl UnmappedMutableGlobalsInstanceSet {
    pub(crate) async fn try_new(
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        mutables_source: &UnmappedLazyBuffer,
        repetitions: usize,
        cap_set: CapabilityStore, // Same as abstract
        head: usize,
    ) -> Result<Self, OutOfMemoryError> {
        Ok(Self {
            mutables: mutables_source
                .try_duplicate_interleave(
                    memory_system,
                    queue,
                    &InterleavedBufferConfig {
                        label: &format!("{}_instance_set", mutables_source.label()),
                        repetitions,
                        usages: wgpu::BufferUsages::STORAGE,
                        locking_size: None,
                        transfer_size: None,
                    },
                )
                .await?,
            cap_set,
            head,
        })
    }

    pub(crate) fn buffer(&self) -> &UnmappedLazyBuffer {
        &self.mutables
    }

    /// Duplicates the data from a given instance into a new buffer
    pub(super) async fn take(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        interleaved_index: usize,
    ) -> Result<(UnmappedLazyBuffer, usize, CapabilityStore), OutOfMemoryError> {
        let buffer = self
            .mutables
            .try_uninterleave(memory_system, queue, interleaved_index)
            .await?;

        Ok((buffer, self.head, self.cap_set.clone()))
    }
}
impl_concrete_ptr!(
    pub struct GlobalMutablePtr {
        data...
    } with abstract AbstractGlobalMutablePtr;
);
