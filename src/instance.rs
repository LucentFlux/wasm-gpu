use crate::externs::{Extern, NamedExtern};
use crate::instance::func::{TypedFuncPtr, UntypedFuncPtr};
use crate::instance::global::builder::AbstractGlobalPtr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use crate::instance::table::builder::AbstractTablePtr;
use crate::module::module_environ::ModuleExport;
use crate::typed::WasmTyVec;
use anyhow::{anyhow, Context};
use std::collections::HashMap;

pub mod data;
pub mod element;
pub mod func;
pub mod global;
pub mod memory;
pub mod ptrs;
pub mod table;

/// Points into a `StoreSetBuilder`, `CompletedBuilder`, `DeviceStoreSet` or `HostStoreSet` to the
/// elements given by the module instantiated to produce this object
pub struct ModuleInstanceReferences {
    funcs: Vec<UntypedFuncPtr>,
    tables: Vec<AbstractTablePtr>,
    memories: Vec<AbstractMemoryPtr>,
    globals: Vec<AbstractGlobalPtr>,
    exports: HashMap<String, ModuleExport>,
    start_fn: Option<UntypedFuncPtr>,
}

impl ModuleInstanceReferences {
    pub fn new(
        funcs: Vec<UntypedFuncPtr>,
        tables: Vec<AbstractTablePtr>,
        memories: Vec<AbstractMemoryPtr>,
        globals: Vec<AbstractGlobalPtr>,
        exports: HashMap<String, ModuleExport>,
        start_fn: Option<UntypedFuncPtr>,
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

    pub fn get_named_exports(&self, module_name: &str) -> Vec<NamedExtern> {
        self.exports
            .keys()
            .into_iter()
            .map(|export_name| {
                let export = self.get_export(export_name).unwrap();
                NamedExtern {
                    module: module_name.to_string(),
                    name: export_name.to_string(),
                    ext: export,
                }
            })
            .collect()
    }

    pub fn get_export(&self, name: &str) -> Option<Extern> {
        let mod_exp = self.exports.get(name)?;
        return Some(match &mod_exp {
            ModuleExport::Func(ptr) => Extern::Func(self.funcs.get(*ptr)?.clone()),
            ModuleExport::Global(ptr) => Extern::Global(self.globals.get(*ptr)?.clone()),
            ModuleExport::Table(ptr) => Extern::Table(self.tables.get(*ptr)?.clone()),
            ModuleExport::Memory(ptr) => Extern::Memory(self.memories.get(*ptr)?.clone()),
        });
    }

    /// Create an exported function that doesn't track its types, useful for runtime imports.
    /// Prefer get_typed_func if possible, and see get_typed_func for detail
    /// about this function.
    pub fn get_func(&self, name: &str) -> anyhow::Result<UntypedFuncPtr> {
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
    ) -> anyhow::Result<TypedFuncPtr<Params, Results>>
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

    pub fn get_memory_export(&self, name: &str) -> anyhow::Result<AbstractMemoryPtr> {
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
