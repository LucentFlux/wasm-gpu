use crate::atomic_counter::AtomicCounter;
use crate::impl_concrete_ptr;
use crate::instance::abstr::memory::AbstractMemoryPtr;
use crate::memory::interleaved::InterleavedBuffer;
use crate::Backend;

static COUNTER: AtomicCounter = AtomicCounter::new();

const STRIDE: usize = 4;

pub struct MemoryInstanceSet<B>
where
    B: Backend,
{
    data: InterleavedBuffer<B, STRIDE>,
    id: usize,
}

impl_concrete_ptr!(
    pub struct MemoryPtr<B: Backend, T> {
        ...
    } with abstract AbstractMemoryPtr<B, T>;
);
