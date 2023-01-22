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
}

impl UnmappedMutableGlobalsInstanceSet {
    pub(crate) async fn new(
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        mutables_source: &UnmappedLazyBuffer,
        count: usize,
        cap_set: CapabilityStore, // Same as abstract
    ) -> Result<Self, OutOfMemoryError> {
        Ok(Self {
            mutables: mutables_source
                .duplicate_interleave(
                    memory_system,
                    queue,
                    &InterleavedBufferConfig {
                        repetitions: count,
                        usages: wgpu::BufferUsages::STORAGE,
                        locking_size: None,
                    },
                )
                .await?,
            cap_set,
        })
    }
}
impl_concrete_ptr!(
    pub struct GlobalMutablePtr<T> {
        data...
    } with abstract AbstractGlobalMutablePtr<T>;
);
