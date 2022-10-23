use crate::impl_concrete_ptr;
use crate::instance::table::abstr::AbstractTablePtr;
use crate::memory::interleaved::{DeviceInterleavedBuffer, HostInterleavedBuffer};
use crate::Backend;

const STRIDE: usize = 1; // FuncRef is 1 x u32

pub struct DeviceTableInstanceSet<B>
where
    B: Backend,
{
    data: DeviceInterleavedBuffer<B, STRIDE>,
    id: usize,
}

pub struct HostTableInstanceSet<B>
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
