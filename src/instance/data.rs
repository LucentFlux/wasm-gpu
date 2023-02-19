use wgpu::BufferAsyncError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    EmptyMemoryBlockConfig, LazilyMappable, MappedLazyBuffer, MemorySystem, UnmappedLazyBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

use crate::capabilities::CapabilityStore;
use crate::impl_immutable_ptr;

#[lazy_mappable(MappedDataInstance)]
#[derive(Debug)]
pub struct UnmappedDataInstance {
    #[map(MappedLazyBuffer)]
    datas: UnmappedLazyBuffer,
    cap_set: CapabilityStore,
    head: usize,
}

impl UnmappedDataInstance {
    /// Used for unit tests. Consumes and gets the contained bytes
    pub(crate) async fn try_read_all(
        self,
        queue: &AsyncQueue,
    ) -> Result<Vec<u8>, BufferAsyncError> {
        self.datas
            .map_lazy()
            .try_read_slice_locking(queue, ..)
            .await
    }
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
        self.datas.resize_lazy(values_size);
        self.cap_set = self.cap_set.resize_ref(self.datas.len())
    }

    pub async fn try_add_data(
        &mut self,
        queue: &AsyncQueue,
        data: &[u8],
    ) -> Result<DataPtr, BufferAsyncError> {
        let start = self.head;
        let end = start + data.len();
        assert!(
            end <= self.datas.len(),
            "not enough space reserved to insert data to device buffer"
        );
        self.datas
            .try_write_slice_locking(queue, start..end, data)
            .await?;
        return Ok(DataPtr::new(start, self.cap_set.get_cap(), data.len()));
    }

    pub async fn try_get(
        &mut self,
        queue: &AsyncQueue,
        ptr: &DataPtr,
    ) -> Result<Vec<u8>, BufferAsyncError> {
        assert!(
            self.cap_set.check(&ptr.cap),
            "data pointer was not valid for this instance"
        );

        let start = ptr.ptr;
        let end = start + ptr.len;
        return self.datas.try_read_slice_locking(queue, start..end).await;
    }

    /// Calls `elem.drop` on the element pointed to. May or may not actually free the memory
    pub async fn drop(&mut self, _ptr: &DataPtr) {
        //TODO: Use this hint
    }
}

impl_immutable_ptr!(
    pub struct DataPtr {
        data...
        len: usize, // In bytes
    }
);

impl DataPtr {
    pub fn to_index(&self) -> wasm_spirv_funcgen::DataIndex {
        wasm_spirv_funcgen::DataIndex::from(self.ptr)
    }
}
