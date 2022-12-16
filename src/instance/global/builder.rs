use crate::atomic_counter::AtomicCounter;
use crate::capabilities::CapabilityStore;
use crate::impl_abstract_ptr;
use crate::instance::func::UntypedFuncPtr;
use crate::instance::global::immutable::GlobalImmutablePtr;
use crate::instance::global::immutable::{
    MappedImmutableGlobalsInstance, UnmappedImmutableGlobalsInstance,
};
use crate::instance::global::instance::GlobalMutablePtr;
use crate::instance::global::instance::UnmappedMutableGlobalInstanceSet;
use crate::module::module_environ::Global;
use crate::typed::{ExternRef, FuncRef, Ieee32, Ieee64, Val, WasmTyVal};
use lf_hal::backend::Backend;
use lf_hal::memory::{MainMemoryBlock, MemoryBlock};
use std::future::join;
use std::mem::size_of;
use std::sync::Arc;
use wasmparser::{GlobalType, Operator, ValType};

pub struct UnmappedMutableGlobalInstanceBuilder<B>
where
    B: Backend,
{
    pub mutable_values: B::DeviceMemoryBlock,

    cap_set: CapabilityStore,
}

impl<B: Backend> UnmappedMutableGlobalInstanceBuilder<B> {
    pub async fn build(&self, count: usize) -> UnmappedMutableGlobalInstanceSet<B> {
        return UnmappedMutableGlobalInstanceSet::new(
            &self.mutable_values,
            count,
            self.cap_set.clone(),
        )
        .await;
    }
}

pub struct MappedMutableGlobalInstanceBuilder<B>
where
    B: Backend,
{
    /// Holds values, some mutable and some immutable by the typing information in the pointer
    mutable_values: B::MainMemoryBlock,
    mutable_values_head: usize,

    cap_set: CapabilityStore,
}

impl<B: Backend> MappedMutableGlobalInstanceBuilder<B> {
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

    async fn push_typed<V, T>(&mut self, v: V) -> AbstractGlobalPtr<B, T>
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

        let mutable_ptr = AbstractGlobalMutablePtr::new(start, self.id, V::VAL_TYPE);
        return AbstractGlobalPtr::Mutable(mutable_ptr);
    }

    pub async fn push<T>(&mut self, val: Val) -> AbstractGlobalPtr<B, T> {
        match val {
            Val::I32(v) => self.push_typed(v).await,
            Val::I64(v) => self.push_typed(v).await,
            Val::F32(v) => self.push_typed(v).await,
            Val::F64(v) => self.push_typed(v).await,
            Val::V128(v) => self.push_typed(v).await,
            Val::FuncRef(v) => self.push_typed(v).await,
            Val::ExternRef(v) => self.push_typed(v).await,
        }
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

    async fn get_val<T, V: WasmTyVal>(&mut self, ptr: &AbstractGlobalMutablePtr<B, T>) -> Val {
        self.get_typed::<T, V>(ptr).await.to_val()
    }

    pub async fn get<T>(&mut self, ptr: &AbstractGlobalMutablePtr<B, T>) -> Val {
        assert_eq!(self.id, ptr.id());

        match &ptr.content_type() {
            ValType::I32 => self.get_val::<T, i32>(ptr).await,
            ValType::I64 => self.get_val::<T, i64>(ptr).await,
            ValType::F32 => self.get_val::<T, Ieee32>(ptr).await,
            ValType::F64 => self.get_val::<T, Ieee64>(ptr).await,
            ValType::V128 => self.get_val::<T, u128>(ptr).await,
            ValType::FuncRef => self.get_val::<T, FuncRef>(ptr).await,
            ValType::ExternRef => self.get_val::<T, ExternRef>(ptr).await,
        }
    }

    pub async fn unmap(self) -> UnmappedMutableGlobalInstanceBuilder<B> {
        assert_eq!(
            self.mutable_values_head,
            self.mutable_values.len(),
            "mutable space reserved but not used"
        );

        let mutable_values = self.mutable_values.unmap().await;

        UnmappedMutableGlobalInstanceBuilder {
            mutable_values,
            cap_set: self.,
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

    fn id(&self) -> usize {
        match self {
            AbstractGlobalPtr::Immutable(ptr) => ptr.id(),
            AbstractGlobalPtr::Mutable(ptr) => ptr.id,
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
