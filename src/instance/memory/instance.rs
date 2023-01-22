use crate::capabilities::CapabilityStore;
use crate::impl_concrete_ptr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use futures::future::join_all;
use wgpu_async::{async_device::OutOfMemoryError, async_queue::AsyncQueue};
use wgpu_lazybuffers::{MemorySystem, UnmappedLazyBuffer};
use wgpu_lazybuffers_interleaving::{
    Interleaveable, InterleavedBufferConfig, MappedInterleavedBuffer, UnmappedInterleavedBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

const STRIDE: u64 = 16; // 4 * u32

#[lazy_mappable(MappedMemoryInstanceSet)]
pub struct UnmappedMemoryInstanceSet {
    #[map(Vec<MappedInterleavedBuffer<STRIDE>>)]
    data: Vec<UnmappedInterleavedBuffer<STRIDE>>,
    cap_set: CapabilityStore,
}

impl UnmappedMemoryInstanceSet {
    pub(crate) async fn new(
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
            .map(|source| source.duplicate_interleave(memory_system, queue, &cfg));

        let memories: Result<_, _> = join_all(memories).await.into_iter().collect();

        Ok(Self {
            data: memories?,
            cap_set,
        })
    }
}

impl_concrete_ptr!(
    pub struct MemoryPtr<T> {
        data...
    } with abstract AbstractMemoryPtr<T>;
);
