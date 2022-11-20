use crate::atomic_counter::AtomicCounter;
use crate::{impl_immutable_ptr, Backend, DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct UnmappedDataInstance<B>
where
    B: Backend,
{
    datas: B::DeviceMemoryBlock,
    id: usize,
}

impl<B: Backend> UnmappedDataInstance<B> {
    pub fn new(backend: &B) -> Result<Self, B::BufferCreationError> {
        Ok(Self {
            datas: backend.try_create_device_memory_block(0, None)?,
            id: COUNTER.next(),
        })
    }

    pub async fn map(
        self,
    ) -> Result<
        MappedDataInstance<B>,
        (
            <B::DeviceMemoryBlock as DeviceMemoryBlock<B>>::MapError,
            Self,
        ),
    > {
        let len = self.datas.len();
        // Try and if we can't, don't
        match self.datas.map().await {
            Err((err, datas)) => Err((err, Self { datas, ..self })),
            Ok(datas) => Ok(MappedDataInstance {
                head: len,
                datas,
                id: self.id,
            }),
        }
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
    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_size is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(
        self,
        values_size: usize,
    ) -> Result<Self, <B::MainMemoryBlock as MainMemoryBlock<B>>::ResizeError> {
        let datas = self.datas.flush_resize(values_size).await?;
        Ok(Self { datas, ..self })
    }

    pub async fn add_data<T>(
        &mut self,
        data: &[u8],
    ) -> Result<DataPtr<B, T>, <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError> {
        let start = self.head;
        let end = start + data.len();
        assert!(
            end <= self.datas.len(),
            "not enough space reserved to insert data to device buffer"
        );

        let slice = self.datas.as_slice_mut(start..end).await?;

        slice.copy_from_slice(data);

        self.head = end;

        return Ok(DataPtr::new(start, self.id, data.len()));
    }

    pub async fn get<T>(
        &mut self,
        ptr: &DataPtr<B, T>,
    ) -> Result<&[u8], <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError> {
        assert_eq!(ptr.id, self.id);

        let start = ptr.ptr;
        let end = start + ptr.len;
        return self.datas.as_slice(start..end).await;
    }

    pub async fn unmap(
        self,
    ) -> Result<
        UnmappedDataInstance<B>,
        (<B::MainMemoryBlock as MainMemoryBlock<B>>::UnmapError, Self),
    > {
        assert_eq!(self.head, self.datas.len(), "space reserved but not used");

        // Try and if we can't, don't
        match self.datas.unmap().await {
            Err((err, datas)) => Err((err, Self { datas, ..self })),
            Ok(datas) => Ok(UnmappedDataInstance { datas, id: self.id }),
        }
    }
}

impl_immutable_ptr!(
    pub struct DataPtr<B: Backend, T> {
        data...
        len: usize, // In bytes
    }
);
