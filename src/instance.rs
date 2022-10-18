use crate::externs::Extern;
use crate::instance::func::{TypedFuncPtr, UntypedFuncPtr};
use crate::instance::global::abstr::AbstractGlobalPtr;
use crate::instance::memory::abstr::AbstractMemoryPtr;
use crate::instance::table::abstr::AbstractTablePtr;
use crate::module::module_environ::ModuleExport;
use crate::read_only::{AppendOnlyVec, ReadOnly};
use crate::typed::WasmTyVec;
use crate::Backend;
use anyhow::{anyhow, Context};
use elsa::sync::FrozenMap;
use itertools::Itertools;
use std::ops::Deref;
use std::sync::Arc;

pub mod data;
pub mod element;
pub mod func;
pub mod global;
pub mod memory;
pub mod ptrs;
pub mod table;

pub struct ModuleInstance<B, T>
where
    B: Backend,
{
    funcs: AppendOnlyVec<UntypedFuncPtr<B, T>>,
    tables: AppendOnlyVec<AbstractTablePtr<B, T>>,
    memories: AppendOnlyVec<AbstractMemoryPtr<B, T>>,
    globals: AppendOnlyVec<AbstractGlobalPtr<B, T>>,
    exports: FrozenMap<String, Arc<ReadOnly<ModuleExport>>>,
    start_fn: Option<UntypedFuncPtr<B, T>>,
}

impl<B, T> ModuleInstance<B, T>
where
    B: Backend,
{
    pub fn new(
        funcs: AppendOnlyVec<UntypedFuncPtr<B, T>>,
        tables: AppendOnlyVec<AbstractTablePtr<B, T>>,
        memories: AppendOnlyVec<AbstractMemoryPtr<B, T>>,
        globals: AppendOnlyVec<AbstractGlobalPtr<B, T>>,
        exports: FrozenMap<String, Arc<ReadOnly<ModuleExport>>>,
        start_fn: Option<UntypedFuncPtr<B, T>>,
    ) -> Self {
        Self {
            funcs,
            tables,
            memories,
            globals,
            exports,
            start_fn,
        }
    }

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
    pub fn get_func(&self, name: &str) -> anyhow::Result<UntypedFuncPtr<B, T>> {
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
        let typed = untyped.typed();

        return Ok(typed);
    }

    pub fn get_memory_export(&self, name: &str) -> anyhow::Result<AbstractMemoryPtr<B, T>> {
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
    fn get_funcs(self, name: &str) -> Vec<anyhow::Result<UntypedFuncPtr<B, T>>>;

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
    fn get_funcs(self, name: &str) -> Vec<anyhow::Result<UntypedFuncPtr<B, T>>> {
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
