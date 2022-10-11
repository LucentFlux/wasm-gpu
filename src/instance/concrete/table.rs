use crate::atomic_counter::AtomicCounter;
use crate::impl_concrete_ptr;
use crate::instance::abstr::table::AbstractTablePtr;
use crate::memory::interleaved::InterleavedBuffer;
use crate::Backend;

static COUNTER: AtomicCounter = AtomicCounter::new();

const STRIDE: usize = 1; // FuncRef is 1 u32

pub struct TableInstanceSet<B>
where
    B: Backend,
{
    data: InterleavedBuffer<B, STRIDE>,
    id: usize,
}

impl_concrete_ptr!(
    pub struct TablePtr<B: Backend, T> {
        ...
    } with abstract AbstractTablePtr<B, T>;
);
