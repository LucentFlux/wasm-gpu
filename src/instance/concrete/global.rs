use crate::memory::StaticMemoryBlock;
use crate::Backend;
use std::sync::Arc;

pub struct GlobalInstance<B>
where
    B: Backend,
{
    // We only need to store the mutable globals per store - the immutable ones can be in a shared buffer
    shared_immutables: Arc<StaticMemoryBlock<B>>,

    mutables: Arc<StaticMemoryBlock<B>>,
    total_instances: usize,
    instance_id: usize,
}

impl<B> GlobalInstance<B>
where
    B: Backend,
{
    pub fn new() -> Self {}
}
