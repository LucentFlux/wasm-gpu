use crate::atomic_counter::AtomicCounter;
use crate::impl_immutable_ptr;
use lf_hal::backend::Backend;
use lf_hal::memory::{DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct UnmappedDataInstance<B>
where
    B: Backend,
{
    datas: B::DeviceMemoryBlock,
    id: usize,
}

impl<B: Backend> UnmappedDataInstance<B> {
    /// Used for unit tests. Consumes and gets the contained bytes
    pub(crate) async fn read_all(self) -> Vec<u8> {
        self.datas.map().await.as_slice(..).await.to_vec()
    }
}

pub struct MappedDataInstance<B>
where
    B: Backend,
{
    datas: B::MainMemoryBlock,
    id: usize,
    head: usize,
}

impl<B: Backend> MappedDataInstance<B> {
    pub async fn new(backend: &B) -> Self {
        Self {
            datas: backend.create_and_map_empty().await,
            id: COUNTER.next(),
            head: 0,
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_size is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.datas.resize(values_size).await
    }

    pub async fn add_data<T>(&mut self, data: &[u8]) -> DataPtr<B, T> {
        let start = self.head;
        let end = start + data.len();
        assert!(
            end <= self.datas.len(),
            "not enough space reserved to insert data to device buffer"
        );

        let slice = self.datas.as_slice_mut(start..end).await;

        slice.copy_from_slice(data);

        self.head = end;

        return DataPtr::new(start, self.id, data.len());
    }

    pub async fn get<T>(&mut self, ptr: &DataPtr<B, T>) -> &[u8] {
        assert_eq!(ptr.id, self.id);

        let start = ptr.ptr;
        let end = start + ptr.len;
        return self.datas.as_slice(start..end).await;
    }

    pub async fn unmap(self) -> UnmappedDataInstance<B> {
        assert_eq!(self.head, self.datas.len(), "space reserved but not used");

        let datas = self.datas.unmap().await;

        UnmappedDataInstance { datas, id: self.id }
    }
}

impl_immutable_ptr!(
    pub struct DataPtr<B: Backend, T> {
        data...
        len: usize, // In bytes
    }
);
