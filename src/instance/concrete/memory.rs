use crate::atomic_counter::AtomicCounter;
use crate::impl_concrete_ptr;
use crate::instance::abstr::memory::AbstractMemoryPtr;
use crate::Backend;

static COUNTER: AtomicCounter = AtomicCounter::new();

impl_concrete_ptr!(
    pub struct MemoryPtr<B: Backend, T> {
        ...
    } with abstract AbstractMemoryPtr<B, T>;
);
