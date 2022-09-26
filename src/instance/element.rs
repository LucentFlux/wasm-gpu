use crate::memory::DynamicMemoryBlock;
use crate::typed::{FuncRef, WasmTyVal, WasmTyVec};
use crate::{impl_ptr, Backend};
use itertools::Itertools;
use std::sync::Arc;

pub struct ElementInstance<B>
where
    B: Backend,
{
    /// Holds pointers that can later be copied into tables
    references: DynamicMemoryBlock<B>,
    len: usize,

    store_id: usize,
}

impl<B> ElementInstance<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>, store_id: usize) -> Self {
        Self {
            references: DynamicMemoryBlock::new(backend, 0, None),
            len: 0,
            store_id,
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.references.extend(values_size).await;
    }

    pub async fn add_element<T>(&mut self, element: &Vec<u32>) -> anyhow::Result<ElementPtr<B, T>> {
        let start = self.len;
        let end = self.len + (element.len() * std::mem::size_of::<u32>());
        assert!(
            end <= self.references.len(),
            "not enough space reserved to insert element to device buffer"
        );

        let slice = self.references.as_slice_mut(start..end).await?;

        slice.copy_from_slice(
            element
                .iter()
                .flat_map(<FuncRef as WasmTyVal>::to_bytes)
                .collect_vec()
                .as_slice(),
        );

        self.len = end;

        return Ok(ElementPtr::new(start, self.store_id, element.len()));
    }

    pub(crate) async fn get(&mut self, ptr: &ElementPtr<B, T>) -> anyhow::Result<&[u8]> {
        let start = ptr.ptr;
        let end = start + (ptr.len * std::mem::size_of::<u32>());
        return self.references.as_slice(start..end).await;
    }
}

impl_ptr!(
    pub struct ElementPtr<B, T> {
        ...
        len: usize,
    }
);
