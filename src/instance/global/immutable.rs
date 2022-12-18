use crate::capabilities::CapabilityStore;
use crate::impl_immutable_ptr;
use crate::instance::global::builder::AbstractGlobalMutablePtr;
use crate::instance::global::{impl_global_get, impl_global_push};
use crate::instance::AbstractGlobalPtr;
use crate::typed::WasmTyVal;
use crate::ExternRef;
use crate::FuncRef;
use crate::Ieee32;
use crate::Ieee64;
use crate::Val;
use lf_hal::backend::Backend;
use lf_hal::memory::{MainMemoryBlock, MemoryBlock};
use std::mem::size_of;
use std::sync::Arc;
use wasmparser::{GlobalType, ValType};

pub struct UnmappedImmutableGlobalsInstance<B>
where
    B: Backend,
{
    immutables: B::DeviceMemoryBlock,
    cap_set: CapabilityStore,
}

impl<B: Backend> UnmappedImmutableGlobalsInstance<B> {}

pub struct MappedImmutableGlobalsInstance<B>
where
    B: Backend,
{
    backend: Arc<B>,
    immutables: B::MainMemoryBlock,
    cap_set: CapabilityStore,
    head: usize,
}

impl<B: Backend> MappedImmutableGlobalsInstance<B> {
    pub async fn new(backend: Arc<B>) -> Self {
        let cap_set = CapabilityStore::new(0);
        let immutables = backend.create_and_map_empty().await;
        Self {
            backend,
            immutables,
            cap_set,
            head: 0,
        }
    }

    /// Resizes the GPU buffers backing these elements by the specified amount.
    ///
    /// values_size is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.immutables.extend(values_size).await;
        self.cap_set = self.cap_set.resize_ref(self.immutables.len())
    }

    pub async fn push_typed<V, T>(&mut self, v: V) -> GlobalImmutablePtr<B, T>
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

        return GlobalImmutablePtr::new(start, self.cap_set.get_cap(), V::VAL_TYPE);
    }

    impl_global_push! {
        pub async fn push<T>(&mut self, val: Val) -> GlobalImmutablePtr<B, T>
    }

    pub async fn get_typed<T, V: WasmTyVal>(&mut self, ptr: &GlobalImmutablePtr<B, T>) -> V {
        assert!(
            self.cap_set.check(&ptr.cap),
            "immutable pointer was not valid for this instance"
        );

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

    impl_global_get! {
        pub async fn get<T>(&mut self, ptr: &GlobalImmutablePtr<B, T>) -> Val
    }

    pub async fn unmap(self) -> UnmappedImmutableGlobalsInstance<B> {
        assert_eq!(
            self.head,
            self.immutables.len(),
            "space reserved but not used"
        );

        let immutables = self.immutables.unmap().await;

        UnmappedImmutableGlobalsInstance {
            immutables,
            cap_set: self.cap_set,
        }
    }
}

impl_immutable_ptr!(
    pub struct GlobalImmutablePtr<B: Backend, T> {
        data...
        content_type: wasmparser::ValType,
    }
);

impl<B: Backend, T> GlobalImmutablePtr<B, T> {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.content_type.eq(&ty.content_type) && !ty.mutable;
    }
}
