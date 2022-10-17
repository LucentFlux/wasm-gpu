use crate::impl_concrete_ptr;
use crate::instance::table::abstr::AbstractTablePtr;
use crate::memory::interleaved::InterleavedBuffer;
use crate::Backend;

const STRIDE: usize = 1; // FuncRef is 1 x u32

pub struct TableInstanceSet<B>
where
    B: Backend,
{
    data: InterleavedBuffer<B, STRIDE>,
    id: usize,
}

impl_concrete_ptr!(
    pub struct TablePtr<B: Backend, T> {
        data...
    } with abstract AbstractTablePtr<B, T>;
);
