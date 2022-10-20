use crate::atomic_counter::AtomicCounter;
use crate::instance::func::UntypedFuncPtr;
use crate::instance::global::concrete::GlobalPtr;
use crate::module::module_environ::{Global, GlobalInit};
use crate::typed::{ExternRef, FuncRef, Ieee32, Ieee64, Val, WasmTyVal, WasmTyVec};
use crate::{impl_abstract_ptr, Backend, MainMemoryBlock, MemoryBlock};
use std::mem::size_of;
use std::sync::Arc;
use wasmparser::{GlobalType, Operator, ValType};

static COUNTER: AtomicCounter = AtomicCounter::new();

pub struct AbstractGlobalInstance<B>
where
    B: Backend,
{
    /// Holds values, some mutable and some immutable by the below typing information
    values: B::MainMemoryBlock,
    values_head: usize,
    types: Vec<GlobalType>,

    id: usize,
}

impl<B> AbstractGlobalInstance<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>) -> Self {
        Self {
            values: DynamicMemoryBlock::new(backend, 0, None),
            values_head: 0,
            types: vec![],
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

    /// Resizes the GPU buffers backing these globals by the specified amounts.
    ///
    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.values.flush_extend(values_size).await;
    }

    pub async fn push(&mut self, val: Val) -> usize {
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

    async fn push_typed<V>(&mut self, v: V) -> usize
    where
        V: WasmTyVec,
    {
        let bytes = v.to_bytes();

        let start = self.values_head;
        let end = start + bytes.len();

        assert!(
            end <= self.values.len(),
            "values buffer was resized too small"
        );

        let slice = self.values.as_slice_mut(start..end).await;

        slice.copy_from_slice(bytes.as_slice());

        self.values_head = end;

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

        assert!(end <= self.values.len().await, "index out of bounds");

        let slice = self.values.as_slice(start..end).await;

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
        global: Global,
        global_imports: &mut impl Iterator<Item = AbstractGlobalPtr<B, T>>,
        module_globals_so_far: &[AbstractGlobalPtr<B, T>],
    ) -> AbstractGlobalPtr<B, T> {
        // Add type info
        self.types.push(global.ty.clone());

        // Initialise
        let pos = match global.initializer {
            GlobalInit::I32Const(v) => self.push_typed(v).await,
            GlobalInit::I64Const(v) => self.push_typed(v).await,
            GlobalInit::F32Const(v) => self.push_typed(Ieee32::from_bits(v)).await,
            GlobalInit::F64Const(v) => self.push_typed(Ieee64::from_bits(v)).await,
            GlobalInit::V128Const(v) => self.push_typed(v).await,
            GlobalInit::RefNullConst => self.push_typed(FuncRef::none()).await,
            GlobalInit::RefFunc(f) => self.push_typed(FuncRef::from_u32(f)).await,
            GlobalInit::GetGlobal(g) => {
                // Gets and clones
                let index = usize::try_from(g).unwrap();
                let ptr: &AbstractGlobalPtr<B, T> = module_globals_so_far.get(index).expect(
                    format!(
                        "global get id {} not in globals processed so far ({})",
                        g,
                        module_globals_so_far.len()
                    )
                    .as_str(),
                );
                assert!(ptr.is_type(&global.ty));

                let val = self.get(ptr).await;
                self.push(val).await
            } /*
              GlobalInit::Import => {
                  // Gets as reference, doesn't clone
                  let ptr = global_imports
                      .next()
                      .ok_or(anyhow!("global import is not within imports"))?;
                  assert_eq!(self.id, ptr.id);
                  assert_eq!(ptr.ty.content_type, global.ty.content_type); // Mutablility doesn't need to match
                  Ok(ptr.ptr)
              }*/
        };

        return AbstractGlobalPtr::new(pos, self.id, global.ty);
    }
}

impl_abstract_ptr!(
    pub struct AbstractGlobalPtr<B: Backend, T> {
        pub(in crate::instance::global) data...
        ty: GlobalType,
    } with concrete GlobalPtr<B, T>;
);

impl<B: Backend, T> AbstractGlobalPtr<B, T> {
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.ty.eq(ty);
    }
}
