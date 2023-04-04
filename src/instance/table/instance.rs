use crate::capabilities::CapabilityStore;
use crate::impl_concrete_ptr;
use crate::instance::table::builder::AbstractTablePtr;
use wgpu_async::{async_device::OutOfMemoryError, async_queue::AsyncQueue};
use wgpu_lazybuffers::{MemorySystem, UnmappedLazyBuffer};
use wgpu_lazybuffers_interleaving::{
    Interleaveable, InterleavedBufferConfig, MappedInterleavedBuffer, UnmappedInterleavedBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

const STRIDE: u64 = 4; // FuncRef is 1 x u32

#[lazy_mappable(MappedTableInstanceSet)]
pub struct UnmappedTableInstanceSet {
    #[map(MappedInterleavedBuffer<STRIDE>)]
    tables: UnmappedInterleavedBuffer<STRIDE>,
    cap_set: CapabilityStore,
}

impl UnmappedTableInstanceSet {
    pub(crate) async fn try_new(
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        source: &UnmappedLazyBuffer,
        count: usize,
        cap_set: CapabilityStore,
    ) -> Result<Self, OutOfMemoryError> {
        let cfg = InterleavedBufferConfig {
            label: &format!("{}_instance_set", source.label()),
            repetitions: count,
            usages: wgpu::BufferUsages::STORAGE,
            locking_size: None,
            transfer_size: None,
        };
        let tables = source
            .try_duplicate_interleave(memory_system, queue, &cfg)
            .await?;

        Ok(Self { tables, cap_set })
    }

    pub(crate) fn buffer(&self) -> &UnmappedLazyBuffer {
        &self.tables
    }
}

impl_concrete_ptr!(
    pub struct TablePtr {
        data...
    } with abstract AbstractTablePtr;
);
