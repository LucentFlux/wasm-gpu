use crate::func::typed::limits_match;
use crate::memory::DynamicMemoryBlock;
use crate::{impl_ptr, Backend};
use itertools::Itertools;
use std::sync::Arc;
use wasmparser::TableType;

/// Context in which an abstract table pointer is valid
pub struct AbstractTableInstanceSet<B>
where
    B: Backend,
{
    store_id: usize,
    backend: Arc<B>,
    tables: Vec<TableInstance<B>>,
}

impl AbstractTableInstanceSet<B> {
    pub fn new(backend: Arc<B>, store_id: usize) -> Self {
        Self {
            store_id,
            backend,
            tables: Vec::new(),
        }
    }

    pub async fn add_table<T>(&mut self, plan: &TableType) -> AbstractTablePtr<B, T> {
        let ptr = self.tables.len();
        self.tables.push(TableInstance::new(
            self.backend.clone(),
            self.store_id,
            plan.initial as usize,
        ));
        return AbstractTablePtr::new(ptr, self.store_id, plan.clone());
    }

    pub async fn initialize<T>(
        &mut self,
        ptr: &AbstractTablePtr<B, T>,
        data: &[u8],
        offset: usize,
    ) -> anyhow::Result<()> {
        assert_eq!(ptr.store_id, self.store_id);

        self.tables.get(ptr.ptr).initialize(data, offset).await
    }
}

pub struct AbstractTableInstance<B>
where
    B: Backend,
{
    /// Holds pointers
    references: DynamicMemoryBlock<B>,
    len: usize,

    store_id: usize,
}

impl<B> AbstractTableInstance<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>, store_id: usize, initial_size: usize) -> Self {
        Self {
            references: DynamicMemoryBlock::new(backend, initial_size, None),
            len: 0,
            store_id,
        }
    }

    pub async fn initialize<T>(&mut self, data: &[u8], offset: usize) -> anyhow::Result<()> {
        return self.references.write(data, offset).await;
    }
}

impl_ptr!(
    pub struct TablePtr<B, T> {
        ...
        // Copied from Table
        ty: TableType,
    }

    impl<B, T> TablePtr<B, T>
    {
        pub fn is_type(&self, ty: &TableType) -> bool {
            self.ty.element_type.eq(&ty.element_type)
                && limits_match(self.ty.initial, self.ty.maximum, ty.initial, ty.maximum)
        }
    }
);
