use crate::atomic_counter::AtomicCounter;
use crate::instance::memory::concrete::MemoryPtr;
use crate::memory::limits_match;
use crate::{impl_abstract_ptr, Backend, DeviceMemoryBlock, MainMemoryBlock};
use std::sync::Arc;
use wasmparser::MemoryType;

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct DeviceAbstractMemoryInstanceSet<B>
where
    B: Backend,
{
    memories: Vec<B::DeviceMemoryBlock>,
    id: usize,
}

pub struct HostAbstractMemoryInstanceSet<B>
where
    B: Backend,
{
    backend: Arc<B>,
    memories: Vec<B::MainMemoryBlock>,
    id: usize,
}

impl<B: Backend> HostAbstractMemoryInstanceSet<B> {
    pub fn new(backend: Arc<B>) -> Self {
        Self {
            id: COUNTER.next(),
            backend,
            memories: Vec::new(),
        }
    }

    pub async fn add_memory<T>(&mut self, plan: &MemoryType) -> AbstractMemoryPtr<B, T> {
        let ptr = self.memories.len();
        self.memories.push(
            self.backend
                .create_device_memory_block(plan.initial as usize, None)
                .map()
                .await,
        );
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

    pub async fn unmap(self) -> DeviceAbstractMemoryInstanceSet<B> {
        let memories = self.memories.into_iter().map(|t| t.unmap());
        let memories = futures::future::join_all(memories).await;

        DeviceAbstractMemoryInstanceSet {
            id: self.id,
            memories,
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
