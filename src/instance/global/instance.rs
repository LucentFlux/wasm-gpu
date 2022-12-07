use crate::impl_concrete_ptr;
use crate::instance::global::builder::AbstractGlobalMutablePtr;
use lf_hal::backend::Backend;
use lf_hal::memory::interleaved::{DeviceInterleavedBuffer, HostInterleavedBuffer};
use lf_hal::memory::DeviceMemoryBlock;
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
        mutables_source: &B::DeviceMemoryBlock,
        count: usize,
        id: usize, // Same as abstract
    ) -> Self {
        Self {
            mutables: mutables_source.interleave(count).await,
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
