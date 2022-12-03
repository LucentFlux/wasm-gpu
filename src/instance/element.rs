use crate::atomic_counter::AtomicCounter;
use crate::typed::{FuncRef, WasmTyVal, WasmTyVec};
use crate::{impl_immutable_ptr, Backend, MainMemoryBlock, MemoryBlock};
use itertools::Itertools;

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct UnmappedElementInstance<B>
where
    B: Backend,
{
    references: B::DeviceMemoryBlock,
    id: usize,
}

impl<B: Backend> UnmappedElementInstance<B> {}

pub struct MappedElementInstance<B>
where
    B: Backend,
{
    references: B::MainMemoryBlock,
    head: usize,
    id: usize,
}

impl<B: Backend> MappedElementInstance<B> {
    pub async fn new(backend: &B) -> Self {
        let references = backend.create_and_map_empty().await;
        Self {
            references,
            id: COUNTER.next(),
            head: 0,
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.references.extend(values_size).await;
    }

    pub async fn add_element<T>(&mut self, element: Vec<Option<u32>>) -> ElementPtr<B, T> {
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

        return ElementPtr::new(start, self.id, element.len());
    }

    pub async fn get<T>(&mut self, ptr: &ElementPtr<B, T>) -> &[u8] {
        assert_eq!(ptr.id, self.id);

        let start = ptr.ptr;
        let end = start + (ptr.len * std::mem::size_of::<u32>());
        return self.references.as_slice(start..end).await;
    }

    pub async fn unmap(self) -> UnmappedElementInstance<B> {
        assert_eq!(
            self.head,
            self.references.len(),
            "space reserved but not used"
        );

        let references = self.references.unmap().await;

        UnmappedElementInstance {
            references,
            id: self.id,
        }
    }
}

impl_immutable_ptr!(
    pub struct ElementPtr<B: Backend, T> {
        data...
        len: usize,
    }
);
