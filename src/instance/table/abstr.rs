use crate::atomic_counter::AtomicCounter;
use crate::instance::table::concrete::TablePtr;
use crate::memory::limits_match;
use crate::{impl_abstract_ptr, Backend, MainMemoryBlock};
use std::sync::Arc;
use wasmparser::TableType;

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct DeviceAbstractTableInstanceSet<B>
where
    B: Backend,
{
    id: usize,
    backend: Arc<B>,
    tables: Vec<DeviceAbstractTableInstance<B>>,
}

pub struct HostAbstractTableInstanceSet<B>
where
    B: Backend,
{
    id: usize,
    backend: Arc<B>,
    tables: Vec<HostAbstractTableInstance<B>>,
}

impl<B> HostAbstractTableInstanceSet<B>
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
        self.tables.push(HostAbstractTableInstance::new(
            &self.backend,
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
            .get_mut(ptr.ptr)
            .unwrap() // This is append only, so having a pointer implies the item exists
            .initialize::<T>(data, offset)
            .await
    }

    pub async fn unmap(self) -> DeviceAbstractTableInstanceSet<B> {
        let tables = self.tables.into_iter().map(|t| t.unmap());
        let tables = futures::future::join_all(tables).await;

        DeviceAbstractTableInstanceSet {
            id: self.id,
            backend: self.backend,
            tables,
        }
    }
}

pub struct DeviceAbstractTableInstance<B>
where
    B: Backend,
{
    references: B::DeviceMemoryBlock,
}

pub struct HostAbstractTableInstance<B>
where
    B: Backend,
{
    references: B::MainMemoryBlock,
}

impl<B> HostAbstractTableInstance<B>
where
    B: Backend,
{
    pub fn new(backend: &B, initial_size: usize) -> Self {
        Self {
            references: backend.create_device_memory_block(initial_size, None).map(),
        }
    }

    pub async fn initialize<T>(&mut self, data: &[u8], offset: usize) {
        self.references.write(data, offset).await;
    }

    pub async fn unmap(self) -> DeviceAbstractTableInstance<B> {
        DeviceAbstractTableInstance {
            references: self.references.unmap().await,
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
