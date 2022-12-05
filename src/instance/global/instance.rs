use crate::impl_concrete_ptr;
use crate::instance::global::builder::AbstractGlobalMutablePtr;
use lib_hal::backend::Backend;
use lib_hal::memory::interleaved::{DeviceInterleavedBuffer, HostInterleavedBuffer};
use std::sync::Arc;

const STRIDE: usize = 1; // 1 * u32

pub struct UnmappedMutableGlobalInstanceSet<B>
where
    B: Backend,
{
    mutables: DeviceInterleavedBuffer<B, STRIDE>,

    id: usize,
}

impl<B> UnmappedMutableGlobalInstanceSet<B>
where
    B: Backend,
{
    pub(crate) async fn new(
        backend: Arc<B>,
        mutables_source: &B::DeviceMemoryBlock,
        count: usize,
        id: usize, // Same as abstract
    ) -> Self {
        Self {
            mutables: DeviceInterleavedBuffer::new_interleaved_from(
                backend,
                mutables_source,
                count,
            )
            .await,
            id,
        }
    }
}

pub struct MappedMutableGlobalInstanceSet<B>
where
    B: Backend,
{
    mutables: HostInterleavedBuffer<B, STRIDE>,

    id: usize,
}

impl_concrete_ptr!(
    pub struct GlobalMutablePtr<B: Backend, T> {
        data...
    } with abstract AbstractGlobalMutablePtr<B, T>;
);
