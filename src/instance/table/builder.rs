use crate::atomic_counter::AtomicCounter;
use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::table::instance::{TablePtr, UnmappedTableInstanceSet};
use lf_hal::backend::Backend;
use lf_hal::memory::limits_match;
use lf_hal::memory::MainMemoryBlock;
use std::sync::Arc;
use wasmparser::TableType;

pub struct UnmappedTableInstanceSetBuilder<B>
where
    B: Backend,
{
    cap_set: CapabilityStore,
    backend: Arc<B>,
    tables: Vec<B::DeviceMemoryBlock>,
}

impl<B: Backend> UnmappedTableInstanceSetBuilder<B> {
    pub async fn build(&self, count: usize) -> UnmappedTableInstanceSet<B> {
        UnmappedTableInstanceSet::new(&self.tables, count, self.cap_set.clone()).await
    }
}

pub struct MappedTableInstanceSetBuilder<B>
where
    B: Backend,
{
    cap_set: CapabilityStore,
    backend: Arc<B>,
    tables: Vec<B::MainMemoryBlock>,
}

impl<B> MappedTableInstanceSetBuilder<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>) -> Self {
        Self {
            cap_set: CapabilityStore::new(0),
            backend,
            tables: Vec::new(),
        }
    }

    pub async fn add_table<T>(&mut self, plan: &TableType) -> AbstractTablePtr<B, T> {
        let ptr = self.tables.len();
        self.tables.push(self.backend.create_and_map_empty().await);
        self.cap_set = self.cap_set.resize_ref(self.tables.len());
        return AbstractTablePtr::new(ptr, self.id, plan.clone());
    }

    pub async fn initialize<T>(
        &mut self,
        ptr: &AbstractTablePtr<B, T>,
        data: &[u8],
        offset: usize,
    ) {
        assert!(
            self.cap_set.check(&ptr.cap),
            "table pointer was not valid for this instance"
        );

        self.tables
            .get_mut(ptr.ptr)
            .unwrap() // This is append only, so having a pointer implies the item exists
            .write(data, offset)
            .await
    }

    pub async fn unmap(self) -> UnmappedTableInstanceSetBuilder<B> {
        let tables = self.tables.into_iter().map(|t| t.unmap());
        let tables = futures::future::join_all(tables)
            .await
            .into_iter()
            .collect();

        UnmappedTableInstanceSetBuilder {
            cap_set: self.cap_set,
            backend: self.backend,
            tables,
        }
    }
}

impl_abstract_ptr!(
    pub struct AbstractTablePtr<B: Backend, T> {
        pub(in crate::instance::table) data...
        // Copied from Table
        ty: TableType,
    } with concrete TablePtr<B, T>;
);

impl<B: Backend, T> AbstractTablePtr<B, T> {
    pub fn is_type(&self, ty: &TableType) -> bool {
        self.ty.element_type.eq(&ty.element_type)
            && limits_match(self.ty.initial, self.ty.maximum, ty.initial, ty.maximum)
    }
}
