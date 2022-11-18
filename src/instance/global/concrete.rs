use crate::instance::global::abstr::AbstractGlobalMutablePtr;
use crate::memory::interleaved::{DeviceInterleavedBuffer, HostInterleavedBuffer};
use crate::{impl_concrete_ptr, Backend};
use std::sync::Arc;

const STRIDE: usize = 1; // 1 * u32

pub struct DeviceMutableGlobalInstanceSet<B>
where
    B: Backend,
{
    mutables: DeviceInterleavedBuffer<B, STRIDE>,

    id: usize,
}

impl<B> DeviceMutableGlobalInstanceSet<B>
where
    B: Backend,
{
    pub async fn new(
        backend: Arc<B>,
        mutables_source: &B::DeviceMemoryBlock,
        count: usize,
        id: usize, // Same as abstract
    ) -> Result<Self, B::BufferCreationError> {
        Ok(Self {
            mutables: DeviceInterleavedBuffer::new_interleaved_from(
                backend,
                mutables_source,
                count,
            )
            .await?,
            id,
        })
    }
}

pub struct HostMutableGlobalInstanceSet<B>
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
