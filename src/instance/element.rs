use crate::capabilities::CapabilityStore;
use crate::impl_immutable_ptr;
use crate::typed::{FuncRef, WasmTyVal, WasmTyVec};
use itertools::Itertools;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    DelayedOutOfMemoryError, MappedLazyBuffer, MemorySystem, UnmappedLazyBuffer,
};

pub struct UnmappedElementInstance {
    references: UnmappedLazyBuffer,
    cap_set: CapabilityStore,
}

impl UnmappedElementInstance {}

pub struct MappedElementInstance {
    references: MappedLazyBuffer,
    head: usize,
    cap_set: CapabilityStore,
}

impl MappedElementInstance {
    pub fn new(memory_system: &MemorySystem, queue: &AsyncQueue) -> Self {
        let references = memory_system.create_and_map_empty(queue);
        Self {
            references,
            cap_set: CapabilityStore::new(0),
            head: 0,
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.references.extend(values_size).await;
        self.cap_set = self.cap_set.resize_ref(self.references.len());
    }

    pub async fn add_element<T>(&mut self, element: Vec<Option<u32>>) -> ElementPtr<T> {
        let start = self.head;
        let end = start + (element.len() * FuncRef::byte_count());
        assert!(
            end <= self.references.len(),
            "not enough space reserved to insert element to device buffer"
        );

        let slice = self.references.as_slice_mut(start..end).await;

        slice.copy_from_slice(
            element
                .iter()
                .flat_map(|v| WasmTyVal::to_bytes(&FuncRef::from(v)))
                .collect_vec()
                .as_slice(),
        );

        self.head = end;

        return ElementPtr::new(start, self.cap_set.get_cap(), element.len());
    }

    pub async fn get<T>(&mut self, ptr: &ElementPtr<T>) -> &[u8] {
        assert!(
            self.cap_set.check(&ptr.cap),
            "element pointer was not valid for this instance"
        );

        let start = ptr.ptr;
        let end = start + (ptr.len * std::mem::size_of::<u32>());
        return self.references.as_slice(start..end).await;
    }

    /// Calls `elem.drop` on the element pointed to. May or may not actually free the memory
    pub async fn drop<T>(&mut self, ptr: &ElementPtr<T>) {
        //TODO - use this optimisation hint
    }

    pub async fn unmap(
        self,
        queue: &AsyncQueue,
    ) -> Result<UnmappedElementInstance, DelayedOutOfMemoryError<Self>> {
        assert_eq!(
            self.head,
            self.references.len(),
            "space reserved but not used"
        );

        let references = self
            .references
            .unmap(queue)
            .await
            .map_oom(|references| Self { references, ..self })?;

        Ok(UnmappedElementInstance {
            references,
            cap_set: self.cap_set,
        })
    }
}

impl_immutable_ptr!(
    pub struct ElementPtr<T> {
        data...
        len: usize,
    }
);
