use crate::atomic_counter::AtomicCounter;
use crate::memory::DynamicMemoryBlock;
use crate::typed::{FuncRef, WasmTyVal};
use crate::{impl_immutable_ptr, Backend};
use itertools::Itertools;
use std::sync::Arc;

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct ElementInstance<B>
where
    B: Backend,
{
    /// Holds pointers that can later be copied into tables
    references: DynamicMemoryBlock<B>,
    len: usize,

    id: usize,
}

impl<B> ElementInstance<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>) -> Self {
        Self {
            references: DynamicMemoryBlock::new(backend, 0, None),
            len: 0,
            id: COUNTER.next(),
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.references.extend(values_size).await;
    }

    pub async fn add_element<T>(&mut self, element: Vec<Option<u32>>) -> ElementPtr<B, T> {
        let start = self.len;
        let end = self.len + (element.len() * std::mem::size_of::<u32>());
        assert!(
            end <= self.references.len().await,
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

        self.len = end;

        return ElementPtr::new(start, self.id, element.len());
    }

    pub(crate) async fn get<T>(&mut self, ptr: &ElementPtr<B, T>) -> &[u8] {
        assert_eq!(ptr.id, self.id);

        let start = ptr.ptr;
        let end = start + (ptr.len * std::mem::size_of::<u32>());
        return self.references.as_slice(start..end).await;
    }
}

impl_immutable_ptr!(
    pub struct ElementPtr<B: Backend, T> {
        data...
        len: usize,
    }
);
