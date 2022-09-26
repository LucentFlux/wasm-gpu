use crate::memory::DynamicMemoryBlock;
use crate::module::module_environ::{Global, GlobalInit};
use crate::store::ptrs::{FuncPtr, StorePtr};
use crate::typed::{ExternRef, FuncRef, Val, WasmTyVal, WasmTyVec};
use crate::{impl_ptr, Backend};
use anyhow::anyhow;
use std::future::join;
use std::io::Write;
use std::mem::size_of;
use std::sync::Arc;
use wasmparser::{GlobalType, Operator, ValType};

pub struct GlobalInstance<B>
where
    B: Backend,
{
    /// Holds values, some mutable and some immutable by the below typing information
    values: DynamicMemoryBlock<B>,
    values_head: usize,
    types: Vec<GlobalType>,

    store_id: usize,
}

impl<B> GlobalInstance<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>, store_id: usize) -> Self {
        Self {
            values: DynamicMemoryBlock::new(backend, 0, None),
            values_head: 0,
            types: vec![],
            store_id,
        }
    }

    /// Used during instantiation to evaluate an expression in a single pass. Only requires this
    pub async fn interpret_constexpr<T>(
        &mut self,
        constr_expr: &Vec<Operator>,
        module_globals: &Vec<GlobalPtr<B, T>>,
        module_functions: &Vec<FuncPtr<B, T>>,
    ) -> Val {
        let mut stack = Vec::new();

        let mut iter = constr_expr.into_iter();
        while let Some(expr) = iter.next() {
            match expr {
                Operator::I32Const { value } => stack.push(Val::I32(*value)),
                Operator::I64Const { value } => stack.push(Val::I64(*value)),
                Operator::F32Const { value } => {
                    stack.push(Val::F32(f32::from_le_bytes(value.bits().to_le_bytes())))
                }
                Operator::F64Const { value } => {
                    stack.push(Val::F64(f64::from_le_bytes(value.bits().to_le_bytes())))
                }
                Operator::V128Const { value } => {
                    stack.push(Val::V128(u128::from_le_bytes(value.bytes().clone())))
                }
                Operator::RefNull { ty } => match ty {
                    ValType::FuncRef => stack.push(Val::FuncRef(FuncRef(0))),
                    ValType::ExternRef => stack.push(Val::ExternRef(ExternRef(0))),
                    _ => unreachable!(),
                },
                Operator::RefFunc { function_index } => {
                    let function_index = usize::try_from(function_index).unwrap();
                    let function_ptr = module_functions
                        .get(function_index)
                        .expect("function index out of range of module functions");
                    stack.push(Val::FuncRef(FuncRef(function_ptr.get_ptr() as u32)))
                }
                Operator::GlobalGet { global_index } => {
                    let global_index = usize::try_from(global_index).unwrap();
                    let global_ptr = module_globals
                        .get(global_index)
                        .expect("global index out of range of module globals");
                    let global_val = self.get(global_ptr).await?;
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
        self.values.extend(values_size).await;
    }

    pub async fn push(&mut self, val: Val) -> anyhow::Result<usize> {
        match val {
            Val::I32(v)
            | Val::I64(v)
            | Val::F32(v)
            | Val::F64(v)
            | Val::V128(v)
            | Val::FuncRef(v)
            | Val::ExternRef(v) => self.push_typed(v).await,
        }
    }

    async fn push_typed<V>(&mut self, v: V) -> anyhow::Result<usize>
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

        let slice = self.values.as_slice_mut(start..end).await?;

        slice.copy_from_slice(bytes.as_slice());

        self.values_head = end;

        return Ok(start);
    }

    pub async fn get<T>(&mut self, ptr: &GlobalPtr<B, T>) -> anyhow::Result<Val> {
        match &ptr.ty.content_type {
            ValType::I32 => self.get_typed::<T, i32>(ptr)?.to_val(),
            ValType::I64 => self.get_typed::<T, i64>(ptr)?.to_val(),
            ValType::F32 => self.get_typed::<T, f32>(ptr)?.to_val(),
            ValType::F64 => self.get_typed::<T, f64>(ptr)?.to_val(),
            ValType::V128 => self.get_typed::<T, u128>(ptr)?.to_val(),
            ValType::FuncRef => self.get_typed::<T, FuncRef>(ptr)?.to_val(),
            ValType::ExternRef => self.get_typed::<T, ExternRef>(ptr)?.to_val(),
        }
    }

    /// A typed version of `get`, panics if types mismatch
    pub async fn get_typed<T, V>(&mut self, ptr: &GlobalPtr<B, T>) -> anyhow::Result<V>
    where
        V: WasmTyVal,
    {
        assert_eq!(ptr.store_id, self.store_id);
        assert!(ptr.ty.content_type.eq(&V::VAL_TYPE));

        let start = index;
        let end = start + size_of::<V>();

        assert!(end <= self.values.len(), "index out of bounds");

        let slice = self.values.as_slice(start..end).await?;

        return V::try_from_bytes(slice);
    }

    pub async fn add_global<T>(
        &mut self,
        global: Global,
        global_imports: &mut impl Iterator<Item = GlobalPtr<B, T>>,
        module_globals_so_far: &[GlobalPtr<B, T>],
    ) -> anyhow::Result<GlobalPtr<B, T>> {
        // Add type info
        self.types.push(global.ty.clone());

        // Initialise
        let pos = match global.initializer {
            GlobalInit::I32Const(v)
            | GlobalInit::I64Const(v)
            | GlobalInit::F32Const(v)
            | GlobalInit::F64Const(v)
            | GlobalInit::V128Const(v) => self.push_typed(v).await,
            // Func refs are offset by 1 so that 0 is null and 1 is the function at index 0
            GlobalInit::RefNullConst => self.push_typed(FuncRef(0)).await,
            GlobalInit::RefFunc(f) => self.push_typed(FuncRef(f + 1)).await,
            GlobalInit::GetGlobal(g) => {
                // Gets and clones
                let ptr: &GlobalPtr<B, T> = module_globals_so_far.get(g).expect(
                    format!(
                        "global get id {} not in globals processed so far ({})",
                        g,
                        module_globals_so_far.len()
                    )
                    .as_str(),
                );
                assert!(ptr.is_type(&global.ty));

                let val = self.get(ptr).await?;
                self.push(val).await
            }
            GlobalInit::Import => {
                // Gets as reference, doesn't clone
                let ptr = global_imports
                    .next()
                    .ok_or(anyhow!("global import is not within imports"))?;
                assert_eq!(ptr.store_id, self.store_id);
                assert_eq!(ptr.ty.content_type, global.ty.content_type); // Mutablility doesn't need to match
                Ok(ptr.ptr)
            }
        }?;

        return Ok(GlobalPtr::new(pos, self.store_id, global_type));
    }
}

impl_ptr!(
    pub struct GlobalPtr<B, T> {
        ...
        // Copied from Global
        ty: GlobalType,
    }
);

impl<B, T> GlobalPtr<B, T>
where
    B: Backend,
{
    pub fn is_type(&self, ty: &GlobalType) -> bool {
        return self.ty.eq(ty);
    }
}
