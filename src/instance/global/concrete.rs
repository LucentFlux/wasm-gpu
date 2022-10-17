use crate::instance::global::abstr::AbstractGlobalPtr;
use crate::memory::interleaved::InterleavedBuffer;
use crate::memory::DynamicMemoryBlock;
use crate::{impl_concrete_ptr, Backend};
use std::sync::Arc;

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
    pub async fn new(
        backend: Arc<B>,
        immutables: Arc<DynamicMemoryBlock<B>>,
        mutables_source: &mut DynamicMemoryBlock<B>,
        count: usize,
        id: usize, // Same as abstract
    ) -> Self {
        Self {
            immutables,
            mutables: InterleavedBuffer::new_interleaved_from(backend, mutables_source, count)
                .await,
            id,
        }
    }
}

impl_concrete_ptr!(
    pub struct GlobalPtr<B: Backend, T> {
        data...
    } with abstract AbstractGlobalPtr<B, T>;
);
