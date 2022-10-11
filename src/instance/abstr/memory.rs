use crate::atomic_counter::AtomicCounter;
use crate::instance::concrete::memory::MemoryPtr;
use crate::memory::{limits_match, DynamicMemoryBlock};
use crate::{impl_abstract_ptr, Backend};
use std::sync::Arc;
use wasmparser::MemoryType;

static COUNTER: AtomicCounter = AtomicCounter::new();

/// Context in which a memory pointer is valid
pub struct AbstractMemoryInstanceSet<B>
where
    B: Backend,
{
    id: usize,
    backend: Arc<B>,
    memories: Vec<DynamicMemoryBlock<B>>,
}

impl<B: Backend> AbstractMemoryInstanceSet<B> {
    pub fn new(backend: Arc<B>) -> Self {
        Self {
            id: COUNTER.next(),
            backend,
            memories: Vec::new(),
        }
    }

    pub async fn add_memory<T>(&mut self, plan: &MemoryType) -> AbstractMemoryPtr<B, T> {
        let ptr = self.memories.len();
        self.memories.push(DynamicMemoryBlock::new(
            self.backend.clone(),
            plan.initial as usize,
            None,
        ));
        return AbstractMemoryPtr::new(ptr, self.id, plan.clone());
    }

    /// # Panics
    /// Panics if the pointer is not for this abstract memory
    pub async fn initialize<T>(
        &mut self,
        ptr: &AbstractMemoryPtr<B, T>,
        data: &[u8],
        offset: usize,
    ) {
        assert_eq!(ptr.id, self.id);

        self.memories
            .get_mut(ptr.ptr as usize)
            .unwrap() // This is append only, so having a pointer implies the item exists
            .write(data, offset)
            .await
    }
}

impl_abstract_ptr!(
    pub struct AbstractMemoryPtr<B: Backend, T> {
        ...
        // Copied from Memory
        ty: MemoryType,
    } with concrete MemoryPtr<B, T>;
);

impl<B, T> AbstractMemoryPtr<B, T> {
    pub fn is_type(&self, ty: &MemoryType) -> bool {
        limits_match(self.ty.initial, self.ty.maximum, ty.initial, ty.maximum)
    }
}
