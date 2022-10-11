use crate::atomic_counter::AtomicCounter;
use crate::instance::abstr::global::AbstractGlobalPtr;
use crate::memory::interleaved::InterleavedBuffer;
use crate::memory::DynamicMemoryBlock;
use crate::{impl_concrete_ptr, Backend};
use std::sync::Arc;
use wasmparser::GlobalType;

static COUNTER: AtomicCounter = AtomicCounter::new();

const STRIDE: usize = 1;

pub struct GlobalInstanceSet<B>
where
    B: Backend,
{
    immutables: Arc<DynamicMemoryBlock<B>>,
    mutables: InterleavedBuffer<B, STRIDE>,

    id: usize,
}

impl<B> GlobalInstanceSet<B>
where
    B: Backend,
{
    pub fn new(
        immutables: Arc<DynamicMemoryBlock<B>>,
        mutables_source: &DynamicMemoryBlock<B>,
        count: usize,
    ) -> Self {
        Self {
            immutables: immutables,
            mutables: InterleavedBuffer::new_interleaved_from(mutables_source, count),
            id: COUNTER.next(),
        }
    }
}

impl_concrete_ptr!(
    pub struct GlobalPtr<B: Backend, T> {
        ...
        ty: GlobalType,
    } with abstract AbstractGlobalPtr<B, T>;
);
