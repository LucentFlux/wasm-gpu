use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    DelayedOutOfMemoryError, MappedLazyBuffer, MemorySystem, UnmappedLazyBuffer,
};

use crate::atomic_counter::AtomicCounter;
use crate::capabilities::CapabilityStore;
use crate::impl_immutable_ptr;

pub struct UnmappedDataInstance {
    datas: UnmappedLazyBuffer,
    cap_set: CapabilityStore,
}

impl UnmappedDataInstance {
    /// Used for unit tests. Consumes and gets the contained bytes
    pub(crate) async fn read_all(self) -> Vec<u8> {
        self.datas.map().await.as_slice(..).await.to_vec()
    }
}

pub struct MappedDataInstance {
    datas: MappedLazyBuffer,
    cap_set: CapabilityStore,
    head: usize,
}

impl MappedDataInstance {
    pub fn new(memory_system: &MemorySystem, queue: &AsyncQueue) -> Self {
        Self {
            datas: memory_system.create_and_map_empty(queue),
            cap_set: CapabilityStore::new(0),
            head: 0,
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_size is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.datas.resize(values_size).await;
        self.cap_set = self.cap_set.resize_ref(self.datas.len())
    }

    pub async fn add_data<T>(&mut self, data: &[u8]) -> DataPtr<T> {
        let start = self.head;
        let end = start + data.len();
        assert!(
            end <= self.datas.len(),
            "not enough space reserved to insert data to device buffer"
        );

        let slice = self.datas.as_slice_mut(start..end).await;

        slice.copy_from_slice(data);

        self.head = end;

        return DataPtr::new(start, self.cap_set.get_cap(), data.len());
    }

    pub async fn get<T>(&mut self, ptr: &DataPtr<T>) -> &[u8] {
        assert!(
            self.cap_set.check(&ptr.cap),
            "data pointer was not valid for this instance"
        );

        let start = ptr.ptr;
        let end = start + ptr.len;
        return self.datas.as_slice(start..end).await;
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

        let datas = self
            .datas
            .unmap(queue)
            .await
            .map_oom(|datas| Self { datas, ..self })?;

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
