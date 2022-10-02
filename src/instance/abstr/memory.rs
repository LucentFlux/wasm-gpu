use crate::memory::DynamicMemoryBlock;
use crate::{impl_ptr, Backend};
use itertools::Itertools;
use std::sync::Arc;
use wasmparser::MemoryType;

/// Context in which a memory pointer is valid
pub struct AbstractMemoryInstanceSet<B>
where
    B: Backend,
{
    store_id: usize,
    backend: Arc<B>,
    memories: Vec<DynamicMemoryBlock<B>>,
}

impl AbstractMemoryInstanceSet<B> {
    pub fn new(backend: Arc<B>, store_id: usize) -> Self {
        Self {
            store_id,
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
        return AbstractMemoryPtr::new(ptr, self.store_id, plan.clone());
    }

    pub async fn initialize<T>(
        &mut self,
        ptr: &AbstractMemoryPtr<B, T>,
        data: &[u8],
        offset: usize,
    ) -> anyhow::Result<()> {
        assert_eq!(ptr.store_id, self.store_id);

        self.memories
            .get_mut(ptr.ptr as usize)
            .write(data, offset)
            .await
    }
}

impl_ptr!(
    pub struct MemoryPtr<B, T> {
        ...
        // Copied from Memory
        ty: MemoryType,
    }

    impl<B, T> MemoryPtr<B, T> {
        pub fn is_type(&self, ty: &MemoryType) -> bool {
            limits_match(self.ty.initial, self.ty.maximum, ty.initial, ty.maximum)
        }
    }
);
