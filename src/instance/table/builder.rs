use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::memory::wasm_limits_match;
use crate::instance::table::instance::{TablePtr, UnmappedTableInstanceSet};
use wasmparser::TableType;
use wgpu_async::async_device::OutOfMemoryError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem, UnmappedLazyBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

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
    pub fn new(memory_system: &MemorySystem) -> Self {
        Self {
            tables: memory_system.create_and_map_empty(&EmptyMemoryBlockConfig {
                usages: wgpu::BufferUsages::empty(),
                locking_size: 128,
            }),
            cap_set: CapabilityStore::new(0),
        }
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

    pub fn to_index(&self) -> wasm_spirv_funcgen::TableIndex {
        wasm_spirv_funcgen::TableIndex::from(self.ptr)
    }
}
