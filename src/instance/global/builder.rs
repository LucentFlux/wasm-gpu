use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::func::UntypedFuncPtr;
use crate::instance::global::immutable::GlobalImmutablePtr;
use crate::instance::global::instance::GlobalMutablePtr;
use crate::instance::global::instance::UnmappedMutableGlobalsInstanceSet;
use crate::instance::global::{impl_global_get, impl_global_push};
use crate::typed::{ExternRef, FuncRef, Ieee32, Ieee64, Val, WasmTyVal};
use lf_hal::backend::Backend;
use lf_hal::memory::{MainMemoryBlock, MemoryBlock};
use std::mem::size_of;
use wasmparser::{GlobalType, ValType};

pub struct UnmappedMutableGlobalsInstanceBuilder<B>
where
    B: Backend,
{
    pub mutable_values: B::DeviceMemoryBlock,

    cap_set: CapabilityStore,
}

impl<B: Backend> UnmappedMutableGlobalsInstanceBuilder<B> {
    pub async fn build(&self, count: usize) -> UnmappedMutableGlobalsInstanceSet<B> {
        return UnmappedMutableGlobalsInstanceSet::new(
            &self.mutable_values,
            count,
            self.cap_set.clone(),
        )
        .await;
    }
}

pub struct MappedMutableGlobalsInstanceBuilder<B>
where
    B: Backend,
{
    /// Holds values, some mutable and some immutable by the typing information in the pointer
    mutable_values: B::MainMemoryBlock,
    mutable_values_head: usize,

    cap_set: CapabilityStore,
}

impl<B: Backend> MappedMutableGlobalsInstanceBuilder<B> {
    pub async fn new(backend: &B) -> Self {
        Self {
            mutable_values: backend.create_and_map_empty().await,
            mutable_values_head: 0,
            cap_set: CapabilityStore::new(0),
        }
    }

    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.mutable_values.extend(values_size).await;
        self.cap_set = self.cap_set.resize_ref(self.mutable_values.len())
    }

    async fn push_typed<V, T>(&mut self, v: V) -> AbstractGlobalMutablePtr<B, T>
    where
        V: WasmTyVal,
    {
        let bytes = v.to_bytes();

        let start = self.mutable_values_head;
        let end = start + bytes.len();

        assert!(end <= self.mutable_values.len(), "index out of bounds");
        let slice = self.mutable_values.as_slice_mut(start..end).await;

        slice.copy_from_slice(bytes.as_slice());

        self.mutable_values_head = end;

        return AbstractGlobalMutablePtr::new(start, self.cap_set.get_cap(), V::VAL_TYPE);
    }

    impl_global_push! {
        pub async fn push<T>(&mut self, val: Val) -> AbstractGlobalMutablePtr<B, T>
    }

    /// A typed version of `get`, panics if types mismatch
    pub async fn get_typed<T, V: WasmTyVal>(&mut self, ptr: &AbstractGlobalMutablePtr<B, T>) -> V {
        assert!(
            self.cap_set.check(&ptr.cap),
            "global mutable pointer was not valid for this instance"
        );
        assert!(ptr.content_type().eq(&V::VAL_TYPE));

        let start = ptr.ptr;
        let end = start + size_of::<V>();

        assert!(end <= self.mutable_values.len(), "index out of bounds");
        let slice = self.mutable_values.as_slice(start..end).await;

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
        pub async fn get<T>(&mut self, ptr: &AbstractGlobalMutablePtr<B, T>) -> Val
    }

    pub async fn unmap(self) -> UnmappedMutableGlobalsInstanceBuilder<B> {
        assert_eq!(
            self.mutable_values_head,
            self.mutable_values.len(),
            "mutable space reserved but not used"
        );

        let mutable_values = self.mutable_values.unmap().await;

        UnmappedMutableGlobalsInstanceBuilder {
            mutable_values,
            cap_set: self.cap_set,
        }
    }
}

impl_abstract_ptr!(
    pub struct AbstractGlobalMutablePtr<B: Backend, T> {
        pub(in crate::instance::global) data...
        content_type: ValType,
    } with concrete GlobalMutablePtr<B, T>;
);

impl<B: Backend, T> AbstractGlobalMutablePtr<B, T> {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.content_type.eq(&ty.content_type) && ty.mutable;
    }
}

#[derive(Debug)]
pub enum AbstractGlobalPtr<B: Backend, T> {
    Immutable(GlobalImmutablePtr<B, T>),
    Mutable(AbstractGlobalMutablePtr<B, T>),
}

impl<B: Backend, T> AbstractGlobalPtr<B, T> {
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

impl<B: Backend, T> Clone for AbstractGlobalPtr<B, T> {
    fn clone(&self) -> Self {
        match self {
            AbstractGlobalPtr::Immutable(ptr) => AbstractGlobalPtr::Immutable(ptr.clone()),
            AbstractGlobalPtr::Mutable(ptr) => AbstractGlobalPtr::Mutable(ptr.clone()),
        }
    }
}
