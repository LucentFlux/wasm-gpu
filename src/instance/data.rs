use crate::atomic_counter::AtomicCounter;
use crate::{impl_immutable_ptr, Backend, DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct DeviceDataInstance<B>
where
    B: Backend,
{
    datas: B::DeviceMemoryBlock,
    id: usize,
}

impl<B: Backend> DeviceDataInstance<B> {
    pub fn new(backend: &B) -> Self {
        Self {
            datas: backend.create_device_memory_block(0, None),
            id: COUNTER.next(),
        }
    }

    pub async fn map(self) -> HostDataInstance<B> {
        HostDataInstance {
            datas: self.datas.map().await,
            id: self.id,
            head: self.id,
        }
    }
}

pub struct HostDataInstance<B>
where
    B: Backend,
{
    datas: B::MainMemoryBlock,
    id: usize,
    head: usize,
}

impl<B: Backend> HostDataInstance<B> {
    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_size is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.datas.flush_resize(values_size).await;
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

    pub async fn unmap(self) -> DeviceDataInstance<B> {
        assert_eq!(self.head, self.datas.len(), "space reserved but not used");

        DeviceDataInstance {
            datas: self.datas.unmap().await,
            id: self.id,
        }
    }
}

impl_immutable_ptr!(
    pub struct DataPtr<B: Backend, T> {
        data...
        len: usize, // In bytes
    }
);
