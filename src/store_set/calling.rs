use std::ops::RangeBounds;
use wgpu::BufferAsyncError;
use wgpu_async::AsyncQueue;

use crate::instance::memory::instance::{MappedMemoryInstanceSet, MemoryView};
use crate::instance::ptrs::AbstractPtr;
use crate::instance::ModuleInstanceReferences;
use crate::store_set::HostStoreSet;

pub struct ActiveMemoryView<'a> {
    view: MemoryView<'a>,
    queue: &'a AsyncQueue,
}

impl<'a> ActiveMemoryView<'a> {
    pub async fn try_read_slice(
        &self,
        slice: impl RangeBounds<usize>,
    ) -> Result<Vec<u8>, BufferAsyncError> {
        self.view.try_read_slice(self.queue, slice).await
    }

    pub async fn try_write_slice(
        &self,
        slice: impl RangeBounds<usize>,
        data: &[u8],
    ) -> Result<(), BufferAsyncError> {
        self.view.try_write_slice(self.queue, slice, data).await
    }
}

/// B is the backend type,
/// T is the data associated with the store_set
pub struct Caller<'a> {
    // Decomposed store
    memory: &'a mut MappedMemoryInstanceSet,

    // Info into store data
    index: usize,
    instance: &'a ModuleInstanceReferences,

    // Action data
    queue: &'a AsyncQueue,
}

impl<'a> Caller<'a> {
    pub fn new(
        stores: &'a mut HostStoreSet,
        index: usize,
        instance: &'a ModuleInstanceReferences,
        queue: &'a AsyncQueue,
    ) -> Self {
        Self {
            memory: &mut stores.owned.memories,

            index,
            instance,

            queue,
        }
    }

    pub async fn get_memory<'b>(&'b self, name: &str) -> Option<ActiveMemoryView<'b>> {
        let memptr = self.instance.get_memory_export(name).ok()?;
        let memptr = memptr.concrete(self.index);

        let view = self.memory.get(&memptr);
        Some(ActiveMemoryView {
            view,
            queue: self.queue,
        })
    }
}
