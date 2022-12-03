use crate::atomic_counter::AtomicCounter;
use crate::instance::memory::instance::{MemoryPtr, UnmappedMemoryInstanceSet};
use crate::memory::limits_match;
use crate::{impl_abstract_ptr, Backend, MainMemoryBlock};
use std::sync::Arc;
use wasmparser::MemoryType;

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct UnmappedMemoryInstanceSetBuilder<B>
where
    B: Backend,
{
    backend: Arc<B>,
    memories: Vec<B::DeviceMemoryBlock>,
    id: usize,
}

impl<B: Backend> UnmappedMemoryInstanceSetBuilder<B> {
    pub async fn build(&self, count: usize) -> UnmappedMemoryInstanceSet<B> {
        UnmappedMemoryInstanceSet::new(self.backend.clone(), &self.memories, count, self.id).await
    }
}

pub struct MappedMemoryInstanceSetBuilder<B>
where
    B: Backend,
{
    backend: Arc<B>,
    memories: Vec<B::MainMemoryBlock>,
    id: usize,
}

impl<B: Backend> MappedMemoryInstanceSetBuilder<B> {
    pub fn new(backend: Arc<B>) -> Self {
        Self {
            id: COUNTER.next(),
            backend,
            memories: Vec::new(),
        }
    }

    pub async fn add_memory<T>(&mut self, plan: &MemoryType) -> AbstractMemoryPtr<B, T> {
        let ptr = self.memories.len();
        self.memories
            .push(self.backend.create_and_map_empty().await);
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

    pub async fn unmap(self) -> UnmappedMemoryInstanceSetBuilder<B> {
        let memories = self.memories.into_iter().map(|t| t.unmap());
        let memories = futures::future::join_all(memories)
            .await
            .into_iter()
            .collect();

        UnmappedMemoryInstanceSetBuilder {
            id: self.id,
            memories,
            backend: self.backend,
        }
    }
}

impl_abstract_ptr!(
    pub struct AbstractMemoryPtr<B: Backend, T> {
        pub(in crate::instance::memory) data...
        // Copied from Memory
        ty: MemoryType,
    } with concrete MemoryPtr<B, T>;
);

impl<B: Backend, T> AbstractMemoryPtr<B, T> {
    pub fn is_type(&self, ty: &MemoryType) -> bool {
        limits_match(self.ty.initial, self.ty.maximum, ty.initial, ty.maximum)
    }
}
