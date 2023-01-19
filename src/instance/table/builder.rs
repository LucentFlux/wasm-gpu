use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::memory::wasm_limits_match;
use crate::instance::table::instance::{TablePtr, UnmappedTableInstanceSet};
use wasmparser::TableType;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    DelayedOutOfMemoryError, EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem,
    UnmappedLazyBuffer,
};

pub struct UnmappedTableInstanceSetBuilder {
    cap_set: CapabilityStore,
    tables: Vec<UnmappedLazyBuffer>,
}

impl UnmappedTableInstanceSetBuilder {
    pub async fn build(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        count: usize,
    ) -> UnmappedTableInstanceSet {
        UnmappedTableInstanceSet::new(
            memory_system,
            queue,
            &self.tables,
            count,
            self.cap_set.clone(),
        )
        .await
    }
}

pub struct MappedTableInstanceSetBuilder {
    cap_set: CapabilityStore,
    tables: Vec<MappedLazyBuffer>,
    memory_system: MemorySystem,
}

impl MappedTableInstanceSetBuilder {
    pub fn new(memory_system: &MemorySystem) -> Self {
        Self {
            cap_set: CapabilityStore::new(0),
            tables: Vec::new(),
            memory_system: memory_system.clone(),
        }
    }

    pub fn add_table<T>(&mut self, plan: &TableType) -> AbstractTablePtr<T> {
        let ptr = self.tables.len();
        self.tables.push(
            self.memory_system
                .create_and_map_empty(&EmptyMemoryBlockConfig {
                    usages: wgpu::BufferUsages::STORAGE,
                    locking_size: None,
                }),
        );
        self.cap_set = self.cap_set.resize_ref(self.tables.len());
        return AbstractTablePtr::new(ptr, self.cap_set.get_cap(), plan.clone());
    }

    pub async fn initialize<T>(&mut self, ptr: &AbstractTablePtr<T>, data: &[u8], offset: usize) {
        assert!(
            self.cap_set.check(&ptr.cap),
            "table pointer was not valid for this instance"
        );

        self.tables
            .get_mut(ptr.ptr)
            .expect("Table builders are append only, so having a pointer implies the item exists")
            .write(data, offset)
            .await
    }

    pub async fn unmap(
        self,
        queue: &AsyncQueue,
    ) -> Result<UnmappedTableInstanceSetBuilder, DelayedOutOfMemoryError<Self>> {
        let tables = self
            .tables
            .unmap_all(queue)
            .map_oom(|tables| Self { tables, ..self })?;

        Ok(UnmappedTableInstanceSetBuilder {
            cap_set: self.cap_set,
            tables,
        })
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
