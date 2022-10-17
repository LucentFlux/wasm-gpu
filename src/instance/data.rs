use crate::atomic_counter::AtomicCounter;
use crate::memory::DynamicMemoryBlock;
use crate::{impl_immutable_ptr, Backend};
use std::sync::Arc;

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct DataInstance<B>
where
    B: Backend,
{
    /// Holds data that can later be copied into memory
    datas: DynamicMemoryBlock<B>,
    len: usize,

    id: usize,
}

impl<B> DataInstance<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>) -> Self {
        Self {
            datas: DynamicMemoryBlock::new(backend, 0, None),
            len: 0,
            id: COUNTER.next(),
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_size is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.datas.extend(values_size).await;
    }

    pub async fn add_data<T>(&mut self, data: &[u8]) -> DataPtr<B, T> {
        let start = self.len;
        let end = start + data.len();
        assert!(
            end <= self.datas.len().await,
            "not enough space reserved to insert data to device buffer"
        );

        let slice = self.datas.as_slice_mut(start..end).await;

        slice.copy_from_slice(data);

        self.len = end;

        return DataPtr::new(start, self.id, data.len());
    }

    pub async fn get<T>(&mut self, ptr: &DataPtr<B, T>) -> &[u8] {
        assert_eq!(ptr.id, self.id);

        let start = ptr.ptr;
        let end = start + ptr.len;
        return self.datas.as_slice(start..end).await;
    }
}

impl_immutable_ptr!(
    pub struct DataPtr<B: Backend, T> {
        data...
        len: usize, // In bytes
    }
);
