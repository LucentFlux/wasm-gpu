use crate::instance::global::abstr::AbstractGlobalPtr;
use crate::memory::interleaved::{DeviceInterleavedBuffer, HostInterleavedBuffer};
use crate::{impl_concrete_ptr, Backend};
use std::sync::Arc;

const STRIDE: usize = 1; // 1 * u32

pub struct DeviceGlobalInstanceSet<B>
where
    B: Backend,
{
    immutables: B::DeviceMemoryBlock,
    mutables: DeviceInterleavedBuffer<B, STRIDE>,

    id: usize,
}

impl<B> DeviceGlobalInstanceSet<B>
where
    B: Backend,
{
    pub async fn new(
        backend: Arc<B>,
        immutables: B::DeviceMemoryBlock,
        mutables_source: &B::DeviceMemoryBlock,
        count: usize,
        id: usize, // Same as abstract
    ) -> Self {
        Self {
            immutables,
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

pub struct HostGlobalInstanceSet<B>
where
    B: Backend,
{
    immutables: B::MainMemoryBlock,
    mutables: HostInterleavedBuffer<B, STRIDE>,

    id: usize,
}

impl_concrete_ptr!(
    pub struct GlobalPtr<B: Backend, T> {
        data...
    } with abstract AbstractGlobalPtr<B, T>;
);
