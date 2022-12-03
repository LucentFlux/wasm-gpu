use crate::atomic_counter::AtomicCounter;
use crate::instance::table::instance::{TablePtr, UnmappedTableInstanceSet};
use crate::memory::limits_match;
use crate::{impl_abstract_ptr, Backend, MainMemoryBlock};
use std::sync::Arc;
use wasmparser::TableType;

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct UnmappedTableInstanceSetBuilder<B>
where
    B: Backend,
{
    id: usize,
    backend: Arc<B>,
    tables: Vec<B::DeviceMemoryBlock>,
}

impl<B: Backend> UnmappedTableInstanceSetBuilder<B> {
    pub async fn build(
        &self,
        count: usize,
    ) -> Result<UnmappedTableInstanceSet<B>, B::BufferCreationError> {
        UnmappedTableInstanceSet::new(self.backend.clone(), &self.tables, count, self.id).await
    }
}

pub struct MappedTableInstanceSetBuilder<B>
where
    B: Backend,
{
    id: usize,
    backend: Arc<B>,
    tables: Vec<B::MainMemoryBlock>,
}

impl<B> MappedTableInstanceSetBuilder<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>) -> Self {
        Self {
            id: COUNTER.next(),
            backend,
            tables: Vec::new(),
        }
    }

    pub async fn add_table<T>(
        &mut self,
        plan: &TableType,
    ) -> Result<AbstractTablePtr<B, T>, AllocOrMapFailure<B>> {
        let ptr = self.tables.len();
        self.tables
            .push(self.backend.try_create_and_map_empty().await?);
        return Ok(AbstractTablePtr::new(ptr, self.id, plan.clone()));
    }

    pub async fn initialize<T>(
        &mut self,
        ptr: &AbstractTablePtr<B, T>,
        data: &[u8],
        offset: usize,
    ) -> Result<(), <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError> {
        assert_eq!(ptr.id, self.id);

        self.tables
            .get_mut(ptr.ptr)
            .unwrap() // This is append only, so having a pointer implies the item exists
            .write(data, offset)
            .await
    }

    pub async fn unmap(
        self,
    ) -> Result<
        UnmappedTableInstanceSetBuilder<B>,
        <B::MainMemoryBlock as MainMemoryBlock<B>>::UnmapError,
    > {
        let tables = self.tables.into_iter().map(|t| t.unmap());
        let tables: Result<Vec<_>, _> = futures::future::join_all(tables)
            .await
            .into_iter()
            .collect();

        Ok(UnmappedTableInstanceSetBuilder {
            id: self.id,
            backend: self.backend,
            tables: tables.map_err(|(e, _)| e)?,
        })
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
