use crate::atomic_counter::AtomicCounter;
use crate::impl_concrete_ptr;
use crate::instance::abstr::table::AbstractTablePtr;
use crate::Backend;

static COUNTER: AtomicCounter = AtomicCounter::new();

impl_concrete_ptr!(
    pub struct TablePtr<B: Backend, T> {
        ...
    } with abstract AbstractTablePtr<B, T>;
);
