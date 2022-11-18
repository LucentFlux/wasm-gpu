use crate::memory::DeviceMemoryBlock;
use crate::typed::WasmTyVal;
use crate::{impl_immutable_ptr, Backend, MainMemoryBlock, MemoryBlock};
use std::mem::size_of;
use wasmparser::{GlobalType, ValType};

pub struct DeviceImmutableGlobalsInstance<B>
where
    B: Backend,
{
    immutables: B::DeviceMemoryBlock,
    id: usize, // Shared with mutable counterpart
}

impl<B: Backend> DeviceImmutableGlobalsInstance<B> {
    pub fn new(backend: &B, id: usize) -> Result<Self, B::BufferCreationError> {
        Ok(Self {
            immutables: backend.try_create_device_memory_block(0, None)?,
            id,
        })
    }

    pub async fn map(
        self,
    ) -> Result<
        HostImmutableGlobalsInstance<B>,
        (
            Self,
            <B::DeviceMemoryBlock as DeviceMemoryBlock<B>>::MapError,
        ),
    > {
        let len = self.immutables.len();

        match self.immutables.map().await {
            Err((err, immutables)) => Err((Self { immutables, ..self }, err)),
            Ok(immutables) => Ok(HostImmutableGlobalsInstance {
                head: len,
                immutables,
                id: self.id,
            }),
        }
    }
}

pub struct HostImmutableGlobalsInstance<B>
where
    B: Backend,
{
    immutables: B::MainMemoryBlock,
    id: usize,
    head: usize,
}

impl<B: Backend> HostImmutableGlobalsInstance<B> {
    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_size is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(
        self,
        values_size: usize,
    ) -> Result<Self, <B::MainMemoryBlock as MainMemoryBlock<B>>::ResizeError> {
        let immutables = self.immutables.flush_extend(values_size).await?;
        Ok(Self { immutables, ..self })
    }

    // Called through joint collection of mutables and immutables
    pub(crate) async fn push_typed<V, T>(
        &mut self,
        v: V,
    ) -> Result<GlobalImmutablePtr<B, T>, <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError>
    where
        V: WasmTyVal,
    {
        let bytes = v.to_bytes();

        let start = self.head;
        let end = start + bytes.len();

        assert!(end <= self.immutables.len(), "index out of bounds");
        let slice = self.immutables.as_slice_mut(start..end).await?;

        slice.copy_from_slice(bytes.as_slice());

        self.head = end;

        return Ok(GlobalImmutablePtr::new(start, self.id, V::VAL_TYPE));
    }

    pub async fn get_typed<T, V: WasmTyVal>(
        &mut self,
        ptr: &GlobalImmutablePtr<B, T>,
    ) -> Result<V, <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError> {
        let start = ptr.ptr;
        let end = start + size_of::<V>();

        assert!(end <= self.immutables.len(), "index out of bounds");
        let slice = self.immutables.as_slice(start..end).await?;

        return Ok(V::try_from_bytes(slice).expect(
            format!(
                "could not parse memory - invalid state for {}: {:?}",
                std::any::type_name::<V>(),
                slice
            )
            .as_str(),
        ));
    }

    pub async fn unmap(
        self,
    ) -> Result<
        DeviceImmutableGlobalsInstance<B>,
        (Self, <B::MainMemoryBlock as MainMemoryBlock<B>>::UnmapError),
    > {
        assert_eq!(
            self.head,
            self.immutables.len(),
            "space reserved but not used"
        );

        match self.immutables.unmap().await {
            Err((err, immutables)) => Err((Self { immutables, ..self }, err)),
            Ok(immutables) => Ok(DeviceImmutableGlobalsInstance {
                immutables,
                id: self.id,
            }),
        }
    }
}

impl_immutable_ptr!(
    pub struct GlobalImmutablePtr<B: Backend, T> {
        data...
        content_type: ValType,
    }
);

impl<B: Backend, T> GlobalImmutablePtr<B, T> {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.content_type.eq(&ty.content_type) && !ty.mutable;
    }

    pub(in crate::instance::global) fn id(&self) -> usize {
        return self.id;
    }
}
