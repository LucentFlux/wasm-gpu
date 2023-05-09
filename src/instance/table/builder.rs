use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::memory::wasm_limits_match;
use crate::instance::table::instance::{TablePtr, UnmappedTableInstanceSet};
use wasmparser::TableType;
use wgpu_async::async_device::OutOfMemoryError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    EmptyMemoryBlockConfig, LazilyMappable, MappedLazyBuffer, MemorySystem, UnmappedLazyBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

use super::instance::MappedTableInstanceSet;

#[derive(Debug, Clone)]
struct Meta {}

#[lazy_mappable(MappedTableInstanceSetBuilder)]
#[derive(Debug)]
pub struct UnmappedTableInstanceSetBuilder {
    #[map(MappedLazyBuffer)]
    tables: UnmappedLazyBuffer,
    cap_set: CapabilityStore,
}

impl UnmappedTableInstanceSetBuilder {
    pub async fn try_build(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        count: usize,
    ) -> Result<UnmappedTableInstanceSet, OutOfMemoryError> {
        UnmappedTableInstanceSet::try_new(
            memory_system,
            queue,
            &self.tables,
            count,
            self.cap_set.clone(),
        )
        .await
    }
}

impl MappedTableInstanceSetBuilder {
    pub fn new(memory_system: &MemorySystem, module_label: &str) -> Self {
        Self {
            tables: memory_system.create_and_map_empty(&EmptyMemoryBlockConfig {
                label: &format!("{}_table_buffer", module_label),
                usages: wgpu::BufferUsages::empty(),
                locking_size: 128,
                transfer_size: 4096,
            }),
            cap_set: CapabilityStore::new(0),
        }
    }

    /// Takes a live buffer and creates a new builder from the given invocation
    pub async fn from_existing(
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        existing: &UnmappedTableInstanceSet,
        interleaved_index: usize,
    ) -> Result<Self, OutOfMemoryError> {
        let (tables, cap_set) = existing
            .take(memory_system, queue, interleaved_index)
            .await?;
        Ok(Self {
            tables: tables.map_lazy(),
            cap_set,
        })
    }

    pub fn add_table(&mut self, plan: &TableType) -> AbstractTablePtr {
        let ptr = self.tables.len();
        let len = usize::try_from(plan.initial)
            .expect("table must be expressable in RAM, but was too big");
        self.tables.extend_lazy(len);
        self.cap_set = self.cap_set.resize_ref(self.tables.len());
        return AbstractTablePtr::new(ptr, self.cap_set.get_cap(), plan.clone(), len);
    }

    pub async fn try_initialize(
        &mut self,
        queue: &AsyncQueue,
        ptr: &AbstractTablePtr,
        data: &[u8],
        offset: usize,
    ) -> Result<(), wgpu::BufferAsyncError> {
        assert!(
            self.cap_set.check(&ptr.cap),
            "table pointer was not valid for this instance"
        );

        assert!(
            ptr.len >= offset + data.len(),
            "cannot slice memory larger than allocated memory space"
        );

        let bounds = (ptr.ptr + offset)..(ptr.ptr + offset + data.len());

        self.tables
            .try_write_slice_locking(queue, bounds, data)
            .await
    }
}

impl_abstract_ptr!(
    pub struct AbstractTablePtr {
        pub(in crate::instance::table) data...
        // Copied from Table
        ty: TableType,
        len: usize,
    } with concrete TablePtr;
);

impl AbstractTablePtr {
    pub fn is_type(&self, ty: &TableType) -> bool {
        self.ty.element_type.eq(&ty.element_type)
            && wasm_limits_match(self.ty.initial, self.ty.maximum, ty.initial, ty.maximum)
    }

    pub fn to_index(&self) -> wasm_gpu_funcgen::TableIndex {
        wasm_gpu_funcgen::TableIndex::from(self.ptr)
    }
}
