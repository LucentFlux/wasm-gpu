use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::memory::wasm_limits_match;
use crate::instance::table::instance::{TablePtr, UnmappedTableInstanceSet};
use wasmparser::TableType;
use wgpu_async::async_device::OutOfMemoryError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::DelayedOutOfMemoryResult;
use wgpu_lazybuffers::MappedLazyBufferIter;
use wgpu_lazybuffers::{
    DelayedOutOfMemoryError, EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem,
    UnmappedLazyBuffer,
};

#[derive(Debug, Clone)]
struct Meta {
    cap_set: CapabilityStore,
    memory_system: MemorySystem,
}

#[derive(Debug)]
pub struct UnmappedTableInstanceSetBuilder {
    tables: Vec<UnmappedLazyBuffer>,
    meta: Meta,
}

impl UnmappedTableInstanceSetBuilder {
    pub async fn build(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        count: usize,
    ) -> Result<UnmappedTableInstanceSet, OutOfMemoryError> {
        UnmappedTableInstanceSet::new(
            memory_system,
            queue,
            &self.tables,
            count,
            self.meta.cap_set.clone(),
        )
        .await
    }

    pub fn map(self) -> MappedTableInstanceSetBuilder {
        let Self { tables, meta } = self;

        MappedTableInstanceSetBuilder {
            tables: tables.into_iter().map(UnmappedLazyBuffer::map).collect(),
            meta,
        }
    }
}

#[derive(Debug)]
pub struct MappedTableInstanceSetBuilder {
    tables: Vec<MappedLazyBuffer>,
    meta: Meta,
}

impl MappedTableInstanceSetBuilder {
    pub fn new(memory_system: &MemorySystem) -> Self {
        Self {
            tables: Vec::new(),
            meta: Meta {
                cap_set: CapabilityStore::new(0),
                memory_system: memory_system.clone(),
            },
        }
    }

    pub fn add_table<T>(&mut self, plan: &TableType) -> AbstractTablePtr<T> {
        let ptr = self.tables.len();
        self.tables.push(
            self.meta
                .memory_system
                .create_and_map_empty(&EmptyMemoryBlockConfig {
                    usages: wgpu::BufferUsages::empty(),
                    locking_size: 128,
                }),
        );
        self.meta.cap_set = self.meta.cap_set.resize_ref(self.tables.len());
        return AbstractTablePtr::new(ptr, self.meta.cap_set.get_cap(), plan.clone());
    }

    pub async fn initialize<T>(
        &mut self,
        queue: &AsyncQueue,
        ptr: &AbstractTablePtr<T>,
        data: &[u8],
        offset: usize,
    ) -> Result<(), wgpu::BufferAsyncError> {
        assert!(
            self.meta.cap_set.check(&ptr.cap),
            "table pointer was not valid for this instance"
        );

        self.tables
            .get_mut(ptr.ptr)
            .expect("Table builders are append only, so having a pointer implies the item exists")
            .write_slice(queue, offset..offset + data.len(), data)
            .await
    }

    pub async fn unmap(
        self,
        queue: &AsyncQueue,
    ) -> Result<UnmappedTableInstanceSetBuilder, DelayedOutOfMemoryError<Self>> {
        let tables = self.tables.unmap_all(queue).await.map_oom(|tables| Self {
            tables,
            meta: self.meta.clone(),
            ..self
        })?;

        Ok(UnmappedTableInstanceSetBuilder {
            meta: self.meta,
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
