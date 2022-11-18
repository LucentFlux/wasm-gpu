use crate::atomic_counter::AtomicCounter;
use crate::backend::AllocOrMapFailure;
use crate::instance::func::UntypedFuncPtr;
use crate::instance::global::concrete::DeviceMutableGlobalInstanceSet;
use crate::instance::global::concrete::GlobalMutablePtr;
use crate::instance::global::immutable::GlobalImmutablePtr;
use crate::instance::global::immutable::{
    DeviceImmutableGlobalsInstance, HostImmutableGlobalsInstance,
};
use crate::module::module_environ::Global;
use crate::typed::{ExternRef, FuncRef, Ieee32, Ieee64, Val, WasmTyVal};
use crate::{impl_abstract_ptr, Backend, DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};
use perfect_derive::perfect_derive;
use std::future::join;
use std::mem::size_of;
use std::sync::Arc;
use thiserror::Error;
use wasmparser::{GlobalType, Operator, ValType};

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct DeviceAbstractGlobalInstance<B>
where
    B: Backend,
{
    pub immutable_values: Arc<DeviceImmutableGlobalsInstance<B>>,
    pub mutable_values: B::DeviceMemoryBlock,

    id: usize,
}

impl<B: Backend> DeviceAbstractGlobalInstance<B> {
    pub async fn build(
        &self,
        backend: Arc<B>,
        count: usize,
    ) -> Result<
        (
            DeviceMutableGlobalInstanceSet<B>,
            Arc<DeviceImmutableGlobalsInstance<B>>,
        ),
        B::BufferCreationError,
    > {
        let mutables =
            DeviceMutableGlobalInstanceSet::new(backend, &self.mutable_values, count, self.id)
                .await;

        return Ok((mutables?, self.immutable_values.clone()));
    }
}

pub struct HostAbstractGlobalInstance<B>
where
    B: Backend,
{
    /// Holds values, some mutable and some immutable by the typing information in the pointer
    immutable_values: HostImmutableGlobalsInstance<B>,
    mutable_values: B::MainMemoryBlock,
    mutable_values_head: usize,

    id: usize,
}

#[derive(Error)]
#[perfect_derive(Debug)]
pub enum InterpretError<B: Backend> {
    #[error("failed to read constant value")]
    ValueReadError(<B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError),
    #[error("malformed constant expression")]
    MalformedExpression(String),
}

impl<B: Backend> HostAbstractGlobalInstance<B> {
    pub async fn new(backend: &B) -> Result<Self, AllocOrMapFailure<B>> {
        let id = COUNTER.next();
        let immutable_values_fut = DeviceImmutableGlobalsInstance::new(backend, id)
            .map_err(AllocOrMapFailure::AllocError)?
            .map();
        let mutable_values_fut = backend
            .try_create_device_memory_block(0, None)
            .map_err(AllocOrMapFailure::AllocError)?
            .map();
        let (immutable_values, mutable_values) =
            join!(immutable_values_fut, mutable_values_fut).await;
        Ok(Self {
            immutable_values: immutable_values.map_err(|(_, e)| AllocOrMapFailure::MapError(e))?,
            mutable_values: mutable_values.map_err(|(e, _)| AllocOrMapFailure::MapError(e))?,
            mutable_values_head: 0,
            id,
        })
    }

    /// Used during instantiation to evaluate an expression in a single pass. Only requires this
    pub async fn interpret_constexpr<'data, T>(
        &mut self,
        constr_expr: &Vec<Operator<'data>>,
        module_globals: &Vec<AbstractGlobalPtr<B, T>>,
        module_functions: &Vec<UntypedFuncPtr<B, T>>,
    ) -> Result<Val, InterpretError<B>> {
        let mut stack = Vec::new();

        let mut iter = constr_expr.into_iter();
        while let Some(expr) = iter.next() {
            match expr {
                Operator::I32Const { value } => stack.push(Val::I32(*value)),
                Operator::I64Const { value } => stack.push(Val::I64(*value)),
                Operator::F32Const { value } => stack.push(Val::F32(Ieee32::from(*value))),
                Operator::F64Const { value } => stack.push(Val::F64(Ieee64::from(*value))),
                Operator::V128Const { value } => {
                    stack.push(Val::V128(u128::from_le_bytes(value.bytes().clone())))
                }
                Operator::RefNull { ty } => match ty {
                    ValType::FuncRef => stack.push(Val::FuncRef(FuncRef::none())),
                    ValType::ExternRef => stack.push(Val::ExternRef(ExternRef::none())),
                    _ => unreachable!(),
                },
                Operator::RefFunc { function_index } => {
                    let function_index = usize::try_from(*function_index).unwrap();
                    let function_ptr = module_functions
                        .get(function_index)
                        .expect("function index out of range of module functions");
                    stack.push(Val::FuncRef(function_ptr.to_func_ref()))
                }
                Operator::GlobalGet { global_index } => {
                    let global_index = usize::try_from(*global_index).unwrap();
                    let global_ptr = module_globals
                        .get(global_index)
                        .expect("global index out of range of module globals");
                    let global_val = self
                        .get(global_ptr)
                        .await
                        .map_err(InterpretError::ValueReadError)?;
                    stack.push(global_val)
                }
                Operator::End => {
                    if !iter.next().is_none() {
                        // End at end
                        return Err(InterpretError::MalformedExpression(
                            "end expression was found before the end of the constexpr".to_string(),
                        ));
                    }
                    break;
                }
                _ => unreachable!(),
            }
        }

        let res = stack.pop().ok_or(InterpretError::MalformedExpression(
            "constexpr had no result".to_string(),
        ))?;

        return Ok(res);
    }

    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve_mutable(
        self,
        values_size: usize,
    ) -> Result<Self, <B::MainMemoryBlock as MainMemoryBlock<B>>::ResizeError> {
        let mutable_values = self.mutable_values.flush_extend(values_size).await?;
        Ok(Self {
            mutable_values,
            ..self
        })
    }

    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve_immutable(
        self,
        values_size: usize,
    ) -> Result<Self, <B::MainMemoryBlock as MainMemoryBlock<B>>::ResizeError> {
        let immutable_values = self.immutable_values.reserve(values_size).await?;
        Ok(Self {
            immutable_values,
            ..self
        })
    }

    async fn push_typed<V, T>(
        &mut self,
        v: V,
        mutable: bool,
    ) -> Result<AbstractGlobalPtr<B, T>, <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError>
    where
        V: WasmTyVal,
    {
        if !mutable {
            let immutable_ptr = self.immutable_values.push_typed(v).await?;
            return Ok(AbstractGlobalPtr::Immutable(immutable_ptr));
        }

        let bytes = v.to_bytes();

        let start = self.mutable_values_head;
        let end = start + bytes.len();

        assert!(end <= self.mutable_values.len(), "index out of bounds");
        let slice = self.mutable_values.as_slice_mut(start..end).await?;

        slice.copy_from_slice(bytes.as_slice());

        self.mutable_values_head = end;

        let mutable_ptr = AbstractGlobalMutablePtr::new(start, self.id, V::VAL_TYPE);
        return Ok(AbstractGlobalPtr::Mutable(mutable_ptr));
    }

    pub async fn push<T>(
        &mut self,
        val: Val,
        mutable: bool,
    ) -> Result<AbstractGlobalPtr<B, T>, <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError>
    {
        match val {
            Val::I32(v) => self.push_typed(v, mutable).await,
            Val::I64(v) => self.push_typed(v, mutable).await,
            Val::F32(v) => self.push_typed(v, mutable).await,
            Val::F64(v) => self.push_typed(v, mutable).await,
            Val::V128(v) => self.push_typed(v, mutable).await,
            Val::FuncRef(v) => self.push_typed(v, mutable).await,
            Val::ExternRef(v) => self.push_typed(v, mutable).await,
        }
    }

    /// A typed version of `get`, panics if types mismatch
    pub async fn get_typed<T, V: WasmTyVal>(
        &mut self,
        ptr: &AbstractGlobalPtr<B, T>,
    ) -> Result<V, <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError> {
        assert_eq!(self.id, ptr.id());
        assert!(ptr.content_type().eq(&V::VAL_TYPE));

        match ptr {
            AbstractGlobalPtr::Immutable(immutable_ptr) => {
                self.immutable_values.get_typed(immutable_ptr).await
            }
            AbstractGlobalPtr::Mutable(mutable_ptr) => {
                let start = mutable_ptr.ptr;
                let end = start + size_of::<V>();

                assert!(end <= self.mutable_values.len(), "index out of bounds");
                let slice = self.mutable_values.as_slice(start..end).await?;

                return Ok(V::try_from_bytes(slice).expect(
                    format!(
                        "could not parse memory - invalid state for {}: {:?}",
                        std::any::type_name::<V>(),
                        slice
                    )
                    .as_str(),
                ));
            }
        }
    }

    async fn get_val<T, V: WasmTyVal>(
        &mut self,
        ptr: &AbstractGlobalPtr<B, T>,
    ) -> Result<Val, <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError> {
        Ok(self.get_typed::<T, V>(ptr).await?.to_val())
    }

    pub async fn get<T>(
        &mut self,
        ptr: &AbstractGlobalPtr<B, T>,
    ) -> Result<Val, <B::MainMemoryBlock as MainMemoryBlock<B>>::SliceError> {
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

    pub async fn add_global<T>(
        &mut self,
        global: Global<'_>,
        module_globals: &Vec<AbstractGlobalPtr<B, T>>,
        module_functions: &Vec<UntypedFuncPtr<B, T>>,
    ) -> Result<AbstractGlobalPtr<B, T>, InterpretError<B>> {
        // Initialise
        let val = self
            .interpret_constexpr(&global.initializer, module_globals, module_functions)
            .await?;
        assert_eq!(
            val.get_type(),
            global.ty.content_type,
            "global evaluation had differing type to definition"
        );
        return Ok(self
            .push(val, global.ty.mutable)
            .await
            .map_err(InterpretError::ValueReadError)?);
    }

    pub async fn unmap(
        self,
    ) -> Result<
        DeviceAbstractGlobalInstance<B>,
        <B::MainMemoryBlock as MainMemoryBlock<B>>::UnmapError,
    > {
        assert_eq!(
            self.mutable_values_head,
            self.mutable_values.len(),
            "mutable space reserved but not used"
        );

        let immutable_values = self.immutable_values.unmap().await.map_err(|(_, e)| e)?;
        let mutable_values = self.mutable_values.unmap().await.map_err(|(e, _)| e)?;

        Ok(DeviceAbstractGlobalInstance {
            immutable_values: Arc::new(immutable_values),
            mutable_values,
            id: self.id,
        })
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
