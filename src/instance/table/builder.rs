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
    #[map(Vec<MappedLazyBuffer>)]
    tables: Vec<UnmappedLazyBuffer>,
    cap_set: CapabilityStore,
    memory_system: MemorySystem,
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
            tables: Vec::new(),
            cap_set: CapabilityStore::new(0),
            memory_system: memory_system.clone(),
        }
    }

    pub fn add_table<T>(&mut self, plan: &TableType) -> AbstractTablePtr<T> {
        let ptr = self.tables.len();
        self.tables.push(
            self.memory_system
                .create_and_map_empty(&EmptyMemoryBlockConfig {
                    usages: wgpu::BufferUsages::empty(),
                    locking_size: 128,
                }),
        );
        self.cap_set = self.cap_set.resize_ref(self.tables.len());
        return AbstractTablePtr::new(ptr, self.cap_set.get_cap(), plan.clone());
    }

    pub async fn try_initialize<T>(
        &mut self,
        queue: &AsyncQueue,
        ptr: &AbstractTablePtr<T>,
        data: &[u8],
        offset: usize,
    ) -> Result<(), wgpu::BufferAsyncError> {
        assert!(
            self.cap_set.check(&ptr.cap),
            "table pointer was not valid for this instance"
        );

        self.tables
            .get_mut(ptr.ptr)
            .expect("Table builders are append only, so having a pointer implies the item exists")
            .try_write_slice_locking(queue, offset..offset + data.len(), data)
            .await
    }
}

impl_abstract_ptr!(
    pub struct AbstractTablePtr<T> {
        pub(in crate::instance::table) data...
        // Copied from Table
        ty: TableType,
    } with concrete TablePtr<T>;
);

impl<T> AbstractTablePtr<T> {
    pub fn is_type(&self, ty: &TableType) -> bool {
        self.ty.element_type.eq(&ty.element_type)
            && wasm_limits_match(self.ty.initial, self.ty.maximum, ty.initial, ty.maximum)
    }
}
