use crate::atomic_counter::AtomicCounter;
use crate::instance::func::UntypedFuncPtr;
use crate::instance::global::concrete::GlobalPtr;
use crate::module::module_environ::Global;
use crate::typed::{ExternRef, FuncRef, Ieee32, Ieee64, Val, WasmTyVal, WasmTyVec};
use crate::{impl_abstract_ptr, Backend, DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};
use std::future::join;
use std::mem::size_of;
use wasmparser::{GlobalType, Operator, ValType};

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct DeviceAbstractGlobalInstance<B>
where
    B: Backend,
{
    immutable_values: B::DeviceMemoryBlock,
    mutable_values: B::DeviceMemoryBlock,

    id: usize,
}

pub struct HostAbstractGlobalInstance<B>
where
    B: Backend,
{
    /// Holds values, some mutable and some immutable by the typing information in the pointer
    immutable_values: B::MainMemoryBlock,
    immutable_values_head: usize,
    mutable_values: B::MainMemoryBlock,
    mutable_values_head: usize,

    id: usize,
}

impl<B: Backend> HostAbstractGlobalInstance<B> {
    pub async fn new(backend: &B) -> Self {
        let immutable_values_fut = backend.create_device_memory_block(0, None).map();
        let mutable_values_fut = backend.create_device_memory_block(0, None).map();
        let (immutable_values, mutable_values) =
            join!(immutable_values_fut, mutable_values_fut).await;
        Self {
            immutable_values,
            immutable_values_head: 0,
            mutable_values,
            mutable_values_head: 0,
            id: COUNTER.next(),
        }
    }

    /// Used during instantiation to evaluate an expression in a single pass. Only requires this
    pub async fn interpret_constexpr<'data, T>(
        &mut self,
        constr_expr: &Vec<Operator<'data>>,
        module_globals: &Vec<AbstractGlobalPtr<B, T>>,
        module_functions: &Vec<UntypedFuncPtr<B, T>>,
    ) -> Val {
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
                    let global_val = self.get(global_ptr).await;
                    stack.push(global_val)
                }
                Operator::End => {
                    assert!(iter.next().is_none()); // End at end
                    break;
                }
                _ => unreachable!(),
            }
        }

        assert_eq!(stack.len(), 1); // We should only have one thing left

        return stack.pop().unwrap();
    }

    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve_mutable(&mut self, values_size: usize) {
        self.mutable_values.flush_extend(values_size).await;
    }

    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve_immutable(&mut self, values_size: usize) {
        self.immutable_values.flush_extend(values_size).await;
    }

    pub async fn push(&mut self, val: Val, mutable: bool) -> usize {
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

    fn get_values_head(&mut self, mutable: bool) -> &mut usize {
        if mutable {
            &mut self.mutable_values_head
        } else {
            &mut self.immutable_values_head
        }
    }

    async fn push_typed<V>(&mut self, v: V, mutable: bool) -> usize
    where
        V: WasmTyVec,
    {
        let bytes = v.to_bytes();

        let start = *self.get_values_head(mutable);
        let end = start + bytes.len();

        let slice = if mutable {
            assert!(end <= self.mutable_values.len(), "index out of bounds");
            self.mutable_values.as_slice_mut(start..end).await
        } else {
            assert!(end <= self.immutable_values.len(), "index out of bounds");
            self.immutable_values.as_slice_mut(start..end).await
        };

        slice.copy_from_slice(bytes.as_slice());

        *self.get_values_head(mutable) = end;

        return start;
    }

    async fn get_val<T, V: WasmTyVal>(&mut self, ptr: &AbstractGlobalPtr<B, T>) -> Val {
        self.get_typed::<T, V>(ptr).await.to_val()
    }

    pub async fn get<T>(&mut self, ptr: &AbstractGlobalPtr<B, T>) -> Val {
        assert_eq!(self.id, ptr.id);

        match &ptr.ty.content_type {
            ValType::I32 => self.get_val::<T, i32>(ptr).await,
            ValType::I64 => self.get_val::<T, i64>(ptr).await,
            ValType::F32 => self.get_val::<T, Ieee32>(ptr).await,
            ValType::F64 => self.get_val::<T, Ieee64>(ptr).await,
            ValType::V128 => self.get_val::<T, u128>(ptr).await,
            ValType::FuncRef => self.get_val::<T, FuncRef>(ptr).await,
            ValType::ExternRef => self.get_val::<T, ExternRef>(ptr).await,
        }
    }

    /// A typed version of `get`, panics if types mismatch
    pub async fn get_typed<T, V: WasmTyVal>(&mut self, ptr: &AbstractGlobalPtr<B, T>) -> V {
        assert_eq!(self.id, ptr.id);
        assert!(ptr.ty.content_type.eq(&V::VAL_TYPE));

        let start = ptr.ptr;
        let end = start + size_of::<V>();

        let slice = if ptr.ty.mutable {
            assert!(end <= self.mutable_values.len(), "index out of bounds");
            self.mutable_values.as_slice(start..end).await
        } else {
            assert!(end <= self.immutable_values.len(), "index out of bounds");
            self.immutable_values.as_slice(start..end).await
        };

        return V::try_from_bytes(slice).expect(
            format!(
                "could not parse memory - invalid state for {}: {:?}",
                std::any::type_name::<V>(),
                slice
            )
            .as_str(),
        );
    }

    pub async fn add_global<T>(
        &mut self,
        global: Global<'_>,
        module_globals: &Vec<AbstractGlobalPtr<B, T>>,
        module_functions: &Vec<UntypedFuncPtr<B, T>>,
    ) -> AbstractGlobalPtr<B, T> {
        // Initialise
        let val = self
            .interpret_constexpr(&global.initializer, module_globals, module_functions)
            .await;
        assert_eq!(
            val.get_type(),
            global.ty.content_type,
            "global evaluation had differing type to definition"
        );
        let pos = self.push(val, global.ty.mutable).await;

        return AbstractGlobalPtr::new(pos, self.id, global.ty);
    }

    pub async fn unmap(self) -> DeviceAbstractGlobalInstance<B> {
        assert_eq!(
            self.immutable_values_head,
            self.immutable_values.len(),
            "immutable space reserved but not used"
        );
        assert_eq!(
            self.mutable_values_head,
            self.mutable_values.len(),
            "mutable space reserved but not used"
        );

        DeviceAbstractGlobalInstance {
            immutable_values: self.immutable_values.unmap().await,
            mutable_values: self.mutable_values.unmap().await,
            id: self.id,
        }
    }
}

impl_abstract_ptr!(
    pub struct AbstractGlobalPtr<B: Backend, T> {
        pub(in crate::instance::global) data...
        ty: GlobalType, // Also used to decide whether to try to read from the mutable or the immutable buffer
    } with concrete GlobalPtr<B, T>;
);

impl<B: Backend, T> AbstractGlobalPtr<B, T> {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.ty.eq(ty);
    }
}
