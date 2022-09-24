use crate::memory::DynamicMemoryBlock;
use crate::store::GlobalPtr;
use crate::typed::wasm_ty_bytes;
use crate::{impl_ptr, Backend};
use anyhow::anyhow;
use std::future::join;
use std::io::Write;
use std::mem::size_of;
use std::sync::Arc;
use wasmtime_environ::{Global, GlobalInit, WasmType};

const GLOBAL_SIZE_UNIT: usize = size_of::<u32>();

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GlobalType {
    pub wasm_ty: WasmType,
    pub mutability: bool,
}

impl GlobalType {
    pub fn is_type(&self, ty: &Global) -> bool {
        return self.mutability.eq(&ty.mutability) && self.wasm_ty.eq(&ty.wasm_ty);
    }
}

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

impl GlobalInstance<B>
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

    /// Resizes the GPU buffers backing these globals by the specified amounts.
    ///
    /// values_count is given in units of bytes, so an f64 is 8 bytes
    pub async fn reserve(&mut self, values_size: usize) {
        self.values.extend(values_size).await;
    }

    async fn push_val<V>(&mut self, v: &[u8]) -> anyhow::Result<usize> {
        let wasm_val_size = size_of::<V>();
        assert!(
            self.values_head * GLOBAL_SIZE_UNIT + wasm_val_size <= self.values.len(),
            "values buffer was resized too small"
        );

        let slice = self
            .values
            .as_slice_mut(self.values_head..(self.values_head + wasm_val_size))
            .await?;

        let i = slice.write(v)?;

        return Ok(i);
    }

    pub async fn add_global<T>(
        &mut self,
        global: &Global,
        global_imports: &mut impl Iterator<Item = GlobalPtr<B, T>>,
    ) -> anyhow::Result<GlobalPtr<B, T>> {
        // Add type info
        let global_type = GlobalType {
            wasm_ty: global.wasm_ty,
            mutability: global.mutability,
        };
        self.types.push(global_type.clone());

        // Initialise
        let pos = match global.initializer {
            GlobalInit::I32Const(v) => self.push_val::<i32>(&i32::to_le_bytes(v)).await,
            GlobalInit::I64Const(v) => self.push_val::<i64>(&i64::to_le_bytes(v)).await,
            GlobalInit::F32Const(v) => self.push_val::<u32>(&u32::to_le_bytes(v)).await,
            GlobalInit::F64Const(v) => self.push_val::<u64>(&u64::to_le_bytes(v)).await,
            GlobalInit::V128Const(v) => self.push_val::<u128>(&u128::to_le_bytes(v)).await,
            // Func refs are offset by 1 so that 0 is null and 1 is the function at index 0
            GlobalInit::RefNullConst => self.push_val::<u32>(&u32::to_le_bytes(0)).await,
            GlobalInit::RefFunc(f) => {
                self.push_val::<u32>(&u32::to_le_bytes(f.as_u32() + 1))
                    .await
            }
            GlobalInit::GetGlobal(g) => {
                // Gets and clones
                unimplemented!()
            }
            GlobalInit::Import => {
                // Gets as reference, doesn't clone
                let ptr = global_imports
                    .next()
                    .ok_or(anyhow!("global import is not within imports"))?;
                assert_eq!(ptr.store_id, self.store_id);
                ptr.ptr
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
    pub fn is_type(&self, ty: &Global) -> bool {
        return self.ty.is_type(ty);
    }
}
