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
pub struct Caller<'a, T> {
    // Decomposed store
    data: &'a mut Vec<T>,
    memory: &'a mut MappedMemoryInstanceSet,

    // Info into store data
    index: usize,
    instance: &'a ModuleInstanceReferences<T>,

    // Action data
    queue: &'a AsyncQueue,
}

impl<'a, T> Caller<'a, T> {
    pub fn new(
        stores: &'a mut HostStoreSet<T>,
        index: usize,
        instance: &'a ModuleInstanceReferences<T>,
        queue: &'a AsyncQueue,
    ) -> Self {
        Self {
            data: &mut stores.data,
            memory: &mut stores.owned.memories,

            index,
            instance,

            queue,
        }
    }

    pub fn data(&self) -> &T {
        return self.data.get(self.index).unwrap();
    }

    pub fn data_mut(&mut self) -> &mut T {
        return self.data.get_mut(self.index).unwrap();
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
