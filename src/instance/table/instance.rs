use crate::impl_concrete_ptr;
use crate::instance::table::builder::AbstractTablePtr;
use futures::future::join_all;
use lib_hal::backend::Backend;
use lib_hal::memory::interleaved::{DeviceInterleavedBuffer, HostInterleavedBuffer};
use std::sync::Arc;

const STRIDE: usize = 1; // FuncRef is 1 x u32

pub struct UnmappedTableInstanceSet<B>
where
    B: Backend,
{
    data: Vec<DeviceInterleavedBuffer<B, STRIDE>>,
    id: usize,
}

impl<B> UnmappedTableInstanceSet<B>
where
    B: Backend,
{
    pub(crate) async fn new(
        backend: Arc<B>,
        sources: &Vec<B::DeviceMemoryBlock>,
        count: usize,
        id: usize,
    ) -> Self {
        let tables = sources.iter().map(|source| {
            DeviceInterleavedBuffer::new_interleaved_from(backend.clone(), source, count)
        });

        let data: Vec<_> = join_all(tables).await.into_iter().collect();

        Self { data, id }
    }
}

pub struct MappedTableInstanceSet<B>
where
    B: Backend,
{
    data: HostInterleavedBuffer<B, STRIDE>,
    id: usize,
}

impl_concrete_ptr!(
    pub struct TablePtr<B: Backend, T> {
        data...
    } with abstract AbstractTablePtr<B, T>;
);
