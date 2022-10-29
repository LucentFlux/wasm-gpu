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
    pub fn new(backend: &B, id: usize) -> Self {
        Self {
            immutables: backend.create_device_memory_block(0, None),
            id,
        }
    }

    pub async fn map(self) -> HostImmutableGlobalsInstance<B> {
        HostImmutableGlobalsInstance {
            head: self.immutables.len(),
            immutables: self.immutables.map().await,
            id: self.id,
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
    pub async fn reserve(&mut self, values_size: usize) {
        self.immutables.flush_resize(values_size).await;
    }

    // Called through joint collection of mutables and immutables
    pub(crate) async fn push_typed<V, T>(&mut self, v: V) -> GlobalImmutablePtr<B, T>
    where
        V: WasmTyVal,
    {
        let bytes = v.to_bytes();

        let start = self.head;
        let end = start + bytes.len();

        assert!(end <= self.immutables.len(), "index out of bounds");
        let slice = self.immutables.as_slice_mut(start..end).await;

        slice.copy_from_slice(bytes.as_slice());

        self.head = end;

        return GlobalImmutablePtr::new(start, self.id, V::VAL_TYPE);
    }

    pub async fn get_typed<T, V: WasmTyVal>(&mut self, ptr: &GlobalImmutablePtr<B, T>) -> V {
        let start = ptr.ptr;
        let end = start + size_of::<V>();

        assert!(end <= self.immutables.len(), "index out of bounds");
        let slice = self.immutables.as_slice(start..end).await;

        return V::try_from_bytes(slice).expect(
            format!(
                "could not parse memory - invalid state for {}: {:?}",
                std::any::type_name::<V>(),
                slice
            )
            .as_str(),
        );
    }

    pub async fn unmap(self) -> DeviceImmutableGlobalsInstance<B> {
        assert_eq!(
            self.head,
            self.immutables.len(),
            "space reserved but not used"
        );

        DeviceImmutableGlobalsInstance {
            immutables: self.immutables.unmap().await,
            id: self.id,
        }
    }
}

impl_immutable_ptr!(
    pub struct GlobalImmutablePtr<B: Backend, T> {
        data...
        ty: ValType,
    }
);

impl<B: Backend, T> GlobalImmutablePtr<B, T> {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.ty.eq(ty.content_type) && !ty.mutable;
    }
}
