use crate::capabilities::CapabilityStore;
use crate::impl_immutable_ptr;
use crate::typed::{FuncRef, WasmTyVal, WasmTyVec};
use itertools::Itertools;
use wgpu::BufferAsyncError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem, UnmappedLazyBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

#[derive(Debug, Clone)]
struct Meta {
    head: usize,
    cap_set: CapabilityStore,
}

#[lazy_mappable(MappedElementInstance)]
#[derive(Debug)]
pub struct UnmappedElementInstance {
    #[map(MappedLazyBuffer)]
    references: UnmappedLazyBuffer,
    meta: Meta,
}

impl UnmappedElementInstance {}

impl MappedElementInstance {
    pub fn new(memory_system: &MemorySystem) -> Self {
        let references = memory_system.create_and_map_empty(&EmptyMemoryBlockConfig {
            usages: wgpu::BufferUsages::STORAGE,
            locking_size: 8192,
        });
        Self {
            references,
            meta: Meta {
                cap_set: CapabilityStore::new(0),
                head: 0,
            },
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub fn reserve(&mut self, values_size: usize) {
        self.references.extend(values_size);
        self.meta.cap_set = self.meta.cap_set.resize_ref(self.references.len());
    }

    pub async fn add_element<T>(
        &mut self,
        queue: &AsyncQueue,
        element: Vec<Option<u32>>,
    ) -> Result<ElementPtr<T>, BufferAsyncError> {
        let start = self.meta.head;
        let end = start + (element.len() * FuncRef::byte_count());
        assert!(
            end <= self.references.len(),
            "not enough space reserved to insert element to device buffer"
        );

        let bytes = element
            .iter()
            .flat_map(|v| WasmTyVal::to_bytes(&FuncRef::from(v)))
            .collect_vec();

        self.references
            .write_slice(queue, start..end, &bytes)
            .await?;

        self.meta.head = end;

        return Ok(ElementPtr::new(
            start,
            self.meta.cap_set.get_cap(),
            element.len(),
        ));
    }

    pub async fn get<T>(
        &mut self,
        queue: &AsyncQueue,
        ptr: &ElementPtr<T>,
    ) -> Result<Vec<u8>, BufferAsyncError> {
        assert!(
            self.meta.cap_set.check(&ptr.cap),
            "element pointer was not valid for this instance"
        );

        let start = ptr.ptr;
        let end = start + (ptr.len * std::mem::size_of::<u32>());
        return self.references.read_slice(queue, start..end).await;
    }

    /// Calls `elem.drop` on the element pointed to. May or may not actually free the memory
    pub async fn drop<T>(&mut self, ptr: &ElementPtr<T>) {
        //TODO - use this optimisation hint
    }
}

impl_immutable_ptr!(
    pub struct ElementPtr<T> {
        data...
        len: usize,
    }
);
