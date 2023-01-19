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
use std::mem::size_of;
use std::sync::Arc;
use wasmparser::{GlobalType, ValType};
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    DelayedOutOfMemoryError, EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem,
    UnmappedLazyBuffer,
};

pub struct UnmappedImmutableGlobalsInstance {
    immutables: UnmappedLazyBuffer,
    cap_set: CapabilityStore,
}

impl UnmappedImmutableGlobalsInstance {}

pub struct MappedImmutableGlobalsInstance {
    immutables: MappedLazyBuffer,
    cap_set: CapabilityStore,
    head: usize,
}

impl MappedImmutableGlobalsInstance {
    pub fn new(memory_system: &MemorySystem) -> Self {
        let cap_set = CapabilityStore::new(0);
        let immutables = memory_system.create_and_map_empty(&EmptyMemoryBlockConfig {
            usages: wgpu::BufferUsages::STORAGE,
            locking_size: 1024,
        });
        Self {
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

    pub async fn push_typed<V, T>(&mut self, v: V) -> GlobalImmutablePtr<T>
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
        pub async fn push<T>(&mut self, val: Val) -> GlobalImmutablePtr<T>
    }

    pub async fn get_typed<T, V: WasmTyVal>(&mut self, ptr: &GlobalImmutablePtr<T>) -> V {
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
        pub async fn get<T>(&mut self, ptr: &GlobalImmutablePtr<T>) -> Val
    }

    pub async fn unmap(
        self,
        queue: &AsyncQueue,
    ) -> Result<UnmappedImmutableGlobalsInstance, DelayedOutOfMemoryError<Self>> {
        assert_eq!(
            self.head,
            self.immutables.len(),
            "space reserved but not used"
        );

        let immutables = self
            .immutables
            .unmap(queue)
            .await
            .map_oom(|immutables| Self { immutables, ..self })?;

        Ok(UnmappedImmutableGlobalsInstance {
            immutables,
            cap_set: self.cap_set,
        })
    }
}

impl_immutable_ptr!(
    pub struct GlobalImmutablePtr<T> {
        data...
        content_type: wasmparser::ValType,
    }
);

impl<T> GlobalImmutablePtr<T> {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.content_type.eq(&ty.content_type) && !ty.mutable;
    }
}
