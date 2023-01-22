use crate::capabilities::CapabilityStore;
use crate::impl_concrete_ptr;
use crate::instance::table::builder::AbstractTablePtr;
use futures::future::join_all;
use wgpu_async::{async_device::OutOfMemoryError, async_queue::AsyncQueue};
use wgpu_lazybuffers::{MemorySystem, UnmappedLazyBuffer};
use wgpu_lazybuffers_interleaving::{
    Interleaveable, InterleavedBufferConfig, MappedInterleavedBuffer, UnmappedInterleavedBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

const STRIDE: u64 = 4; // FuncRef is 1 x u32

#[lazy_mappable(MappedTableInstanceSet)]
pub struct UnmappedTableInstanceSet {
    #[map(Vec<MappedInterleavedBuffer<STRIDE>>)]
    data: Vec<UnmappedInterleavedBuffer<STRIDE>>,
    cap_set: CapabilityStore,
}

impl UnmappedTableInstanceSet {
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
        let tables = sources
            .iter()
            .map(|source| source.duplicate_interleave(memory_system, queue, &cfg));

        let tables: Result<_, _> = join_all(tables).await.into_iter().collect();

        Ok(Self {
            data: tables?,
            cap_set,
        })
    }
}

impl_concrete_ptr!(
    pub struct TablePtr<T> {
        data...
    } with abstract AbstractTablePtr<T>;
);
