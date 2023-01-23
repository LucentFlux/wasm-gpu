use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::global::immutable::GlobalImmutablePtr;
use crate::instance::global::instance::GlobalMutablePtr;
use crate::instance::global::instance::UnmappedMutableGlobalsInstanceSet;
use crate::instance::global::{impl_global_get, impl_global_push};
use crate::typed::{ExternRef, FuncRef, Ieee32, Ieee64, Val, WasmTyVal};
use std::mem::size_of;
use wasmparser::{GlobalType, ValType};
use wgpu::BufferAsyncError;
use wgpu_async::async_device::OutOfMemoryError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{
    EmptyMemoryBlockConfig, MappedLazyBuffer, MemorySystem, UnmappedLazyBuffer,
};
use wgpu_lazybuffers_macros::lazy_mappable;

#[lazy_mappable(MappedMutableGlobalsInstanceBuilder)]
#[derive(Debug)]
pub struct UnmappedMutableGlobalsInstanceBuilder {
    #[map(MappedLazyBuffer)]
    mutable_values: UnmappedLazyBuffer,

    head: usize,
    cap_set: CapabilityStore,
}

impl UnmappedMutableGlobalsInstanceBuilder {
    pub async fn try_build(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        count: usize,
    ) -> Result<UnmappedMutableGlobalsInstanceSet, OutOfMemoryError> {
        return UnmappedMutableGlobalsInstanceSet::try_new(
            memory_system,
            queue,
            &self.mutable_values,
            count,
            self.cap_set.clone(),
        )
        .await;
    }
}

impl MappedMutableGlobalsInstanceBuilder {
    pub fn new(memory_system: &MemorySystem) -> Self {
        Self {
            mutable_values: memory_system.create_and_map_empty(&EmptyMemoryBlockConfig {
                usages: wgpu::BufferUsages::empty(),
                locking_size: 1024,
            }),
            head: 0,
            cap_set: CapabilityStore::new(0),
        }
    }

    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub fn reserve(&mut self, values_size: usize) {
        self.mutable_values.extend_lazy(values_size);
        self.cap_set = self.cap_set.resize_ref(self.mutable_values.len())
    }

    async fn try_push_typed<V, T>(
        &mut self,
        queue: &AsyncQueue,
        v: V,
    ) -> Result<AbstractGlobalMutablePtr<T>, BufferAsyncError>
    where
        V: WasmTyVal,
    {
        let bytes = v.to_bytes();

        let start = self.head;
        let end = start + bytes.len();

        assert!(end <= self.mutable_values.len(), "index out of bounds");
        self.mutable_values
            .try_write_slice_locking(queue, start..end, bytes.as_slice())
            .await?;

        self.head = end;

        return Ok(AbstractGlobalMutablePtr::new(
            start,
            self.cap_set.get_cap(),
            V::VAL_TYPE,
        ));
    }

    impl_global_push! {
        pub async fn try_push<T>(&mut self, queue: &AsyncQueue, val: Val) -> Result<AbstractGlobalMutablePtr<T>, BufferAsyncError>
    }

    /// A typed version of `get`, panics if types mismatch
    pub async fn try_get_typed<T, V: WasmTyVal>(
        &mut self,
        queue: &AsyncQueue,
        ptr: &AbstractGlobalMutablePtr<T>,
    ) -> Result<V, BufferAsyncError> {
        assert!(
            self.cap_set.check(&ptr.cap),
            "global mutable pointer was not valid for this instance"
        );
        assert!(ptr.content_type().eq(&V::VAL_TYPE));

        let start = ptr.ptr;
        let end = start + size_of::<V>();

        assert!(end <= self.mutable_values.len(), "index out of bounds");
        let bytes = self
            .mutable_values
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
        pub async fn try_get<T>(&mut self, queue: &AsyncQueue, ptr: &AbstractGlobalMutablePtr<T>) -> Result<Val, BufferAsyncError>
    }
}

impl_abstract_ptr!(
    pub struct AbstractGlobalMutablePtr<T> {
        pub(in crate::instance::global) data...
        content_type: ValType,
    } with concrete GlobalMutablePtr<T>;
);

impl<T> AbstractGlobalMutablePtr<T> {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.content_type.eq(&ty.content_type) && ty.mutable;
    }
}

#[derive(Debug)]
pub enum AbstractGlobalPtr<T> {
    Immutable(GlobalImmutablePtr<T>),
    Mutable(AbstractGlobalMutablePtr<T>),
}

impl<T> AbstractGlobalPtr<T> {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        match self {
            AbstractGlobalPtr::Immutable(ptr) => ptr.is_type(ty),
            AbstractGlobalPtr::Mutable(ptr) => ptr.is_type(ty),
        }
    }

    pub fn content_type(&self) -> &ValType {
        match self {
            AbstractGlobalPtr::Immutable(ptr) => ptr.content_type(),
            AbstractGlobalPtr::Mutable(ptr) => ptr.content_type(),
        }
    }

    pub fn mutable(&self) -> bool {
        match self {
            AbstractGlobalPtr::Immutable(_) => false,
            AbstractGlobalPtr::Mutable(_) => true,
        }
    }

    pub fn ty(&self) -> GlobalType {
        GlobalType {
            content_type: *self.content_type(),
            mutable: self.mutable(),
        }
    }
}

impl<T> Clone for AbstractGlobalPtr<T> {
    fn clone(&self) -> Self {
        match self {
            AbstractGlobalPtr::Immutable(ptr) => AbstractGlobalPtr::Immutable(ptr.clone()),
            AbstractGlobalPtr::Mutable(ptr) => AbstractGlobalPtr::Mutable(ptr.clone()),
        }
    }
}
