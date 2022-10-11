use crate::atomic_counter::AtomicCounter;
use crate::instance::abstr::global::AbstractGlobalPtr;
use crate::memory::interleaved::InterleavedBuffer;
use crate::memory::DynamicMemoryBlock;
use crate::{impl_concrete_ptr, Backend};
use std::sync::Arc;
use wasmparser::GlobalType;

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct GlobalInstanceSet<B, const STRIDE: usize>
where
    B: Backend,
{
    shared_immutables: Arc<DynamicMemoryBlock<B>>,

    mutables: InterleavedBuffer<B, STRIDE>,
    total_instances: usize,
    id: usize,
}

impl<B, const STRIDE: usize> GlobalInstanceSet<B, STRIDE>
where
    B: Backend,
{
    pub fn new() -> Self {}
}

impl_concrete_ptr!(
    pub struct GlobalPtr<B: Backend, T> {
        ...
        ty: GlobalType,
    } with abstract AbstractGlobalPtr<B, T>;
);
