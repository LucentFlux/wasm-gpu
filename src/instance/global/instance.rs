use crate::capabilities::CapabilityStore;
use crate::impl_concrete_ptr;
use crate::instance::global::builder::AbstractGlobalMutablePtr;
use lf_hal::backend::Backend;
use lf_hal::memory::interleaved::{DeviceInterleavedBuffer, HostInterleavedBuffer};
use lf_hal::memory::DeviceMemoryBlock;

const STRIDE: usize = 1; // 1 * u32

pub struct UnmappedMutableGlobalsInstanceSet<B>
where
    B: Backend,
{
    mutables: DeviceInterleavedBuffer<B, STRIDE>,

    cap_set: CapabilityStore,
}

impl<B> UnmappedMutableGlobalsInstanceSet<B>
where
    B: Backend,
{
    pub(crate) async fn new(
        mutables_source: &B::DeviceMemoryBlock,
        count: usize,
        cap_set: CapabilityStore, // Same as abstract
    ) -> Self {
        Self {
            mutables: mutables_source.interleave(count).await,
            cap_set,
        }
    }
}

pub struct MappedMutableGlobalsInstanceSet<B>
where
    B: Backend,
{
    mutables: HostInterleavedBuffer<B, STRIDE>,

    cap_set: CapabilityStore,
}

impl_concrete_ptr!(
    pub struct GlobalMutablePtr<B: Backend, T> {
        data...
    } with abstract AbstractGlobalMutablePtr<B, T>;
);
