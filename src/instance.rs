use crate::externs::Extern;
use crate::func::TypedFuncPtr;
use crate::global_instance::GlobalPtr;
use crate::memory::Memory;
use crate::read_only::{AppendOnlyVec, ReadOnly};
use crate::store::ptrs::{FuncPtr, MemoryPtr, TablePtr};
use crate::typed::WasmTyVec;
use crate::{Backend, FuncPtr};
use anyhow::{anyhow, Context};
use elsa::sync::{FrozenMap, FrozenVec};
use itertools::Itertools;
use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;

pub enum ModuleExport {
    Func(usize),
    Table(usize),
    Memory(usize),
    Global(usize),
}

pub struct ModuleInstance<B, T>
where
    B: Backend,
{
    store_id: usize,
    funcs: AppendOnlyVec<FuncPtr<B, T>>,
    tables: AppendOnlyVec<TablePtr<B, T>>,
    memories: AppendOnlyVec<MemoryPtr<B, T>>,
    globals: AppendOnlyVec<GlobalPtr<B, T>>,
    exports: FrozenMap<String, Arc<ReadOnly<ModuleExport>>>,
}

impl<B, T> ModuleInstance<B, T>
where
    B: Backend,
{
    pub fn get_export(&self, name: &str) -> Option<Extern<B, T>> {
        let mod_exp = self.exports.get(name)?;
        return Some(match mod_exp.read().unwrap().deref() {
            ModuleExport::Func(ptr) => Extern::Func(self.funcs.get(*ptr)?.clone()),
            ModuleExport::Global(ptr) => Extern::Global(self.globals.get(*ptr)?.clone()),
            ModuleExport::Table(ptr) => Extern::Table(self.tables.get(*ptr)?.clone()),
            ModuleExport::Memory(ptr) => Extern::Memory(self.memories.get(*ptr)?.clone()),
        });
    }

    /// Create an exported function that doesn't track its types, useful for runtime imports.
    /// Prefer get_typed_func if possible, and see get_typed_func for detail
    /// about this function.
    pub fn get_func(&self, name: &str) -> anyhow::Result<FuncPtr<B, T>> {
        self.get_export(name)
            .ok_or(anyhow!("no exported object with name {}", name))
            .and_then(|export| match export {
                Extern::Func(f) => Ok(f.clone()),
                _ => Err(anyhow!("exported object named {} is not a function", name)),
            })
    }

    /// Create an exported function that tracks its types.
    /// Prefer calling once and reusing the returned exported function.
    pub fn get_typed_func<Params, Results>(
        &self,
        name: &str,
    ) -> anyhow::Result<TypedFuncPtr<B, T, Params, Results>>
    where
        Params: WasmTyVec,
        Results: WasmTyVec,
    {
        let untyped = self
            .get_func(name)
            .context(format!("failed to find function export `{}`", name))?;
        let typed = TypedFuncPtr::<B, T, Params, Results>::try_from(untyped).context(format!(
            "failed to convert function `{}` to given type",
            name
        ))?;

        return Ok(typed);
    }

    pub fn get_memory_export(&self, name: &str) -> anyhow::Result<MemoryPtr<B, T>> {
        self.get_export(name)
            .ok_or(anyhow!("no exported object with name {}", name))
            .and_then(|export| match export {
                Extern::Memory(m) => Ok(m.clone()),
                _ => Err(anyhow!(
                    "exported object named {} is not a memory block",
                    name
                )),
            })
    }
}

pub trait InstanceSet<B, T>
where
    B: Backend,
{
    fn get_funcs(self, name: &str) -> Vec<anyhow::Result<FuncPtr<B, T>>>;

    fn get_typed_funcs<Params, Results>(
        self,
        name: &str,
    ) -> Vec<anyhow::Result<TypedFuncPtr<B, T, Params, Results>>>
    where
        Params: WasmTyVec,
        Results: WasmTyVec;
}

impl<'a, V, B: 'a, T: 'a> InstanceSet<B, T> for V
where
    V: IntoIterator<Item = &'a ModuleInstance<B, T>>,
    B: Backend,
{
    fn get_funcs(self, name: &str) -> Vec<anyhow::Result<FuncPtr<B, T>>> {
        self.into_iter()
            .map(|s: &'a ModuleInstance<B, T>| s.get_func(name))
            .collect_vec()
    }

    fn get_typed_funcs<Params, Results>(
        self,
        name: &str,
    ) -> Vec<anyhow::Result<TypedFuncPtr<B, T, Params, Results>>>
    where
        Params: WasmTyVec,
        Results: WasmTyVec,
    {
        self.into_iter()
            .map(|instance: &'a ModuleInstance<B, T>| instance.get_typed_func(name))
            .collect_vec()
    }
}
