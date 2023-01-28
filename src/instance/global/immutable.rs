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
        self.immutables.extend_lazy(values_size);
        self.cap_set = self.cap_set.resize_ref(self.immutables.len())
    }

    pub async fn try_push_typed<V>(
        &mut self,
        queue: &AsyncQueue,
        v: V,
    ) -> Result<GlobalImmutablePtr, BufferAsyncError>
    where
        V: WasmTyVal,
    {
        let bytes = v.to_bytes();

        let start = self.head;
        let end = start + bytes.len();

        assert!(end <= self.immutables.len(), "index out of bounds");
        self.immutables
            .try_write_slice_locking(queue, start..end, bytes.as_slice())
            .await?;

        self.head = end;

        return Ok(GlobalImmutablePtr::new(
            start,
            self.cap_set.get_cap(),
            V::VAL_TYPE,
        ));
    }

    impl_global_push! {
        pub async fn try_push(&mut self, queue: &AsyncQueue, val: Val) -> Result<GlobalImmutablePtr, BufferAsyncError>
    }

    pub async fn try_get_typed<V: WasmTyVal>(
        &mut self,
        queue: &AsyncQueue,
        ptr: &GlobalImmutablePtr,
    ) -> Result<V, BufferAsyncError> {
        assert!(
            self.cap_set.check(&ptr.cap),
            "immutable pointer was not valid for this instance"
        );

        let start = ptr.ptr;
        let end = start + size_of::<V>();

        assert!(end <= self.immutables.len(), "index out of bounds");
        let bytes = self
            .immutables
            .try_read_slice_locking(queue, start..end)
            .await?;

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
        pub async fn try_get(&mut self,
            queue: &AsyncQueue,ptr: &GlobalImmutablePtr) -> Result<Val, BufferAsyncError>
    }
}

impl_immutable_ptr!(
    pub struct GlobalImmutablePtr {
        data...
        content_type: wasmparser::ValType,
    }
);

impl GlobalImmutablePtr {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.content_type.eq(&ty.content_type) && !ty.mutable;
    }
}
