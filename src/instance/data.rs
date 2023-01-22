use wgpu::BufferAsyncError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::DelayedOutOfMemoryResult;
use wgpu_lazybuffers::{
    DelayedOutOfMemoryError, EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem,
    UnmappedLazyBuffer,
};

use crate::capabilities::CapabilityStore;
use crate::impl_immutable_ptr;

#[derive(Debug)]
pub struct UnmappedDataInstance {
    datas: UnmappedLazyBuffer,
    cap_set: CapabilityStore,
}

impl UnmappedDataInstance {
    /// Used for unit tests. Consumes and gets the contained bytes
    pub(crate) async fn read_all(self, queue: &AsyncQueue) -> Result<Vec<u8>, BufferAsyncError> {
        self.datas.map().read_slice(queue, ..).await
    }
}

#[derive(Debug)]
pub struct MappedDataInstance {
    datas: MappedLazyBuffer,
    cap_set: CapabilityStore,
    head: usize,
}

impl MappedDataInstance {
    pub fn new(memory_system: &MemorySystem) -> Self {
        Self {
            datas: memory_system.create_and_map_empty(&EmptyMemoryBlockConfig {
                usages: wgpu::BufferUsages::STORAGE,
                locking_size: 8192,
            }),
            cap_set: CapabilityStore::new(0),
            head: 0,
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_size is given in units of bytes, so an f64 is 8 bytes
    pub fn reserve(&mut self, values_size: usize) {
        self.datas.resize(values_size);
        self.cap_set = self.cap_set.resize_ref(self.datas.len())
    }

    pub async fn add_data<T>(
        &mut self,
        queue: &AsyncQueue,
        data: &[u8],
    ) -> Result<DataPtr<T>, BufferAsyncError> {
        let start = self.head;
        let end = start + data.len();
        assert!(
            end <= self.datas.len(),
            "not enough space reserved to insert data to device buffer"
        );
        self.datas.write_slice(queue, start..end, data).await?;
        return Ok(DataPtr::new(start, self.cap_set.get_cap(), data.len()));
    }

    pub async fn get<T>(
        &mut self,
        queue: &AsyncQueue,
        ptr: &DataPtr<T>,
    ) -> Result<Vec<u8>, BufferAsyncError> {
        assert!(
            self.cap_set.check(&ptr.cap),
            "data pointer was not valid for this instance"
        );

        let start = ptr.ptr;
        let end = start + ptr.len;
        return self.datas.read_slice(queue, start..end).await;
    }

    /// Calls `elem.drop` on the element pointed to. May or may not actually free the memory
    pub async fn drop<T>(&mut self, ptr: &DataPtr<T>) {
        //TODO: Use this hint
    }

    pub async fn unmap(
        self,
        queue: &AsyncQueue,
    ) -> Result<UnmappedDataInstance, DelayedOutOfMemoryError<Self>> {
        assert_eq!(self.head, self.datas.len(), "space reserved but not used");

        let datas = self.datas.unmap(queue).await.map_oom(|datas| Self {
            datas,
            cap_set: self.cap_set.clone(),
            ..self
        })?;

        return Ok(UnmappedDataInstance {
            datas,
            cap_set: self.cap_set,
        });
    }
}

impl_immutable_ptr!(
    pub struct DataPtr<T> {
        data...
        len: usize, // In bytes
    }
);
