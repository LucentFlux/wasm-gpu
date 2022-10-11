use crate::atomic_counter::AtomicCounter;
use crate::instance::concrete::table::TablePtr;
use crate::memory::limits_match;
use crate::memory::DynamicMemoryBlock;
use crate::{impl_abstract_ptr, Backend};
use std::sync::Arc;
use wasmparser::TableType;

static COUNTER: AtomicCounter = AtomicCounter::new();

/// Context in which an abstr table pointer is valid
pub struct AbstractTableInstanceSet<B>
where
    B: Backend,
{
    id: usize,
    backend: Arc<B>,
    tables: Vec<AbstractTableInstance<B>>,
}

impl<B> AbstractTableInstanceSet<B>
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

    pub async fn add_table<T>(&mut self, plan: &TableType) -> AbstractTablePtr<B, T> {
        let ptr = self.tables.len();
        self.tables.push(AbstractTableInstance::new(
            self.backend.clone(),
            plan.initial as usize,
        ));
        return AbstractTablePtr::new(ptr, self.id, plan.clone());
    }

    pub async fn initialize<T>(
        &mut self,
        ptr: &AbstractTablePtr<B, T>,
        data: &[u8],
        offset: usize,
    ) {
        assert_eq!(ptr.id, self.id);

        self.tables
            .get(ptr.ptr)
            .unwrap() // This is append only, so having a pointer implies the item exists
            .initialize(data, offset)
            .await
    }
}

pub struct AbstractTableInstance<B>
where
    B: Backend,
{
    /// Holds pointers
    references: DynamicMemoryBlock<B>,
    len: usize,
}

impl<B> AbstractTableInstance<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>, initial_size: usize) -> Self {
        Self {
            references: DynamicMemoryBlock::new(backend, initial_size, None),
            len: 0,
        }
    }

    pub async fn initialize<T>(&mut self, data: &[u8], offset: usize) {
        self.references.write(data, offset).await;
    }
}

impl_abstract_ptr!(
    pub struct AbstractTablePtr<B: Backend, T> {
        ...
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
