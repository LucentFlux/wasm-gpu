use crate::capabilities::CapabilityStore;
use crate::impl_immutable_ptr;
use crate::instance::global::{impl_global_get, impl_global_push};
use crate::typed::WasmTyVal;
use crate::ExternRef;
use crate::FuncRef;
use crate::Ieee32;
use crate::Ieee64;
use crate::Val;
use std::mem::size_of;
use wasmparser::{GlobalType, ValType};
use wgpu::BufferAsyncError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem, UnmappedLazyBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

#[derive(Debug, Clone)]
struct Meta {}

#[lazy_mappable(MappedImmutableGlobalsInstance)]
#[derive(Debug)]
pub struct UnmappedImmutableGlobalsInstance {
    #[map(MappedLazyBuffer)]
    immutables: UnmappedLazyBuffer,
    head: usize,
    cap_set: CapabilityStore,
}

impl UnmappedImmutableGlobalsInstance {}

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
    pub fn reserve(&mut self, values_size: usize) {
        self.immutables.extend(values_size);
        self.cap_set = self.cap_set.resize_ref(self.immutables.len())
    }

    pub async fn push_typed<V, T>(
        &mut self,
        queue: &AsyncQueue,
        v: V,
    ) -> Result<GlobalImmutablePtr<T>, BufferAsyncError>
    where
        V: WasmTyVal,
    {
        let bytes = v.to_bytes();

        let start = self.head;
        let end = start + bytes.len();

        assert!(end <= self.immutables.len(), "index out of bounds");
        let slice = self
            .immutables
            .write_slice(queue, start..end, bytes.as_slice())
            .await?;

        self.head = end;

        return Ok(GlobalImmutablePtr::new(
            start,
            self.cap_set.get_cap(),
            V::VAL_TYPE,
        ));
    }

    impl_global_push! {
        pub async fn push<T>(&mut self, queue: &AsyncQueue, val: Val) -> Result<GlobalImmutablePtr<T>, BufferAsyncError>
    }

    pub async fn get_typed<T, V: WasmTyVal>(
        &mut self,
        queue: &AsyncQueue,
        ptr: &GlobalImmutablePtr<T>,
    ) -> Result<V, BufferAsyncError> {
        assert!(
            self.cap_set.check(&ptr.cap),
            "immutable pointer was not valid for this instance"
        );

        let start = ptr.ptr;
        let end = start + size_of::<V>();

        assert!(end <= self.immutables.len(), "index out of bounds");
        let bytes = self.immutables.read_slice(queue, start..end).await?;

        return Ok(V::try_from_bytes(&bytes).expect(
            format!(
                "could not parse memory - invalid state for {}: {:?}",
                std::any::type_name::<V>(),
                bytes
            )
            .as_str(),
        ));
    }

    impl_global_get! {
        pub async fn get<T>(&mut self,
            queue: &AsyncQueue,ptr: &GlobalImmutablePtr<T>) -> Result<Val, BufferAsyncError>
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
