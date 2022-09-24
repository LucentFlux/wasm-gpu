use crate::externs::NamedExtern;
use crate::global_instance::{GlobalInstance, GlobalPtr};
use crate::store::ptrs::{FuncPtr, MemoryPtr, TablePtr};
use crate::typed::wasm_ty_bytes;
use crate::{Backend, Engine, Extern};
use anyhow::{anyhow, Context, Error};
use itertools::Itertools;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::ops::Index;
use std::slice::Iter;
use std::sync::Arc;
use wasmparser::types::{EntityType, TypeId};
use wasmparser::Validator;
use wasmtime_environ::{
    EntityIndex, FuncIndex, FunctionType, Global, GlobalIndex, Initializer, MemoryIndex,
    MemoryPlan, ModuleEnvironment, ModuleTranslation, ModuleTypes, SignatureIndex, TableIndex,
    TablePlan, WasmFuncType,
};

/// A wasm module that has not been instantiated
pub struct Module<'a, B>
where
    B: Backend,
{
    backend: Arc<B>,
    source: Cow<'a, [u8]>,
    translation: ModuleTranslation<'a>,
    types: ModuleTypes,
    validator: Validator,
}

pub struct ValidatedImports<B, T>
where
    B: Backend,
{
    functions: Vec<FuncPtr<B, T>>,
    globals: Vec<GlobalPtr<B, T>>,
    tables: Vec<TablePtr<B, T>>,
    memories: Vec<MemoryPtr<B, T>>,
}

impl<B, T> ValidatedImports<B, T>
where
    B: Backend,
{
    pub fn functions(&self) -> Iter<FuncPtr<B, T>> {
        self.functions.iter()
    }
    pub fn globals(&self) -> Iter<GlobalPtr<B, T>> {
        self.globals.iter()
    }
    pub fn tables(&self) -> Iter<TablePtr<B, T>> {
        self.tables.iter()
    }
    pub fn memories(&self) -> Iter<MemoryPtr<B, T>> {
        self.memories.iter()
    }
}

impl<'a, B> Module<'a, B>
where
    B: Backend,
{
    pub fn new(engine: &Engine<B>, bytes: impl AsRef<[u8]>) -> Result<Self, Error> {
        let bytes = bytes.as_ref();
        let wasm: Cow<'a, [u8]> = wat::parse_bytes(bytes)?;

        let mut validator = Validator::new_with_features(engine.config().features.clone());
        let parser = wasmparser::Parser::new(0);
        let mut types = Default::default();
        let translation =
            ModuleEnvironment::new(&engine.config().tunables, &mut validator, &mut types)
                .translate(parser, &wasm)
                .context("failed to parse WebAssembly module")?;
        let types = types.finish();

        // TODO: Typecheck and compile functions

        return Ok(Self {
            backend: engine.backend(),
            source: wasm,
            translation,
            types,
            validator,
        });
    }

    /// Returns an iterator of all the imports in this module
    pub(crate) fn initializers(&self) -> Vec<Initializer> {
        return self.translation.module.initializers.clone();
    }

    /// Extends a globals memory buffer and indirection buffer to fit the globals contained in this
    /// module, then writes the initial values
    pub(crate) async fn initialize_globals<T>(
        &self,
        globals_instance: &mut GlobalInstance<B>,
        globals: impl Iterator<Item = GlobalPtr<B, T>>,
    ) -> anyhow::Result<BTreeMap<GlobalIndex, GlobalPtr<B, T>>> {
        let globals = globals.collect_vec();

        // Make space for the values
        let values_len: usize = self
            .translation
            .module
            .globals
            .iter()
            .map(|(_, g)| wasm_ty_bytes(g.wasm_ty))
            .sum();
        let values_len = values_len - globals.len(); // The imports don't take any space
        globals_instance.reserve(values_len);

        // Sort globals by index
        let sorted: BTreeMap<GlobalIndex, Global> =
            self.translation.module.globals.iter().collect();

        // Add the values
        let mut imports_iter = globals.into_iter();
        let mut futures = Vec::new();
        for (global_index, global) in sorted.into_iter() {
            let future = globals_instance
                .add_global(global, &mut imports_iter)
                .map(move |v| (global_index, v));
            futures.push(future);
        }

        // Await all - this should be fast because the only thing to wait for is to map the memory the first time
        let results: anyhow::Result<BTreeMap<GlobalIndex, GlobalPtr<B, T>>> =
            futures::future::join_all(futures)
                .await
                .into_iter()
                .collect();

        return results;
    }

    fn extern_is_type<T>(&self, provided: &Extern<B, T>, ty: EntityIndex) -> bool {
        match (provided, ty) {
            (Extern::Func(f1), EntityIndex::Function(f2)) => {
                let ty = self.translation.module.functions.get(f2);
                let ty = match ty {
                    None => return false,
                    Some(ty) => ty,
                };
                let ty = self.types.index(ty.signature);
                f1.is_type(ty)
            }
            (Extern::Memory(m1), EntityIndex::Memory(m2)) => {
                let ty = self.translation.module.memory_plans.get(m2);
                let ty = match ty {
                    None => return false,
                    Some(ty) => ty,
                };
                m1.is_type(ty)
            }
            (Extern::Global(g1), EntityIndex::Global(g2)) => {
                let ty = self.translation.module.globals.get(g2);
                let ty = match ty {
                    None => return false,
                    Some(ty) => ty,
                };
                g1.is_type(ty)
            }
            (Extern::Table(t1), EntityIndex::Table(t2)) => {
                let ty = self.translation.module.table_plans.get(t2);
                let ty = match ty {
                    None => return false,
                    Some(ty) => ty,
                };
                t1.is_type(ty)
            }
            (_, _) => false,
        }
    }

    /// See 4.5.4 of WASM spec 2.0
    /// Performs 1-4
    pub fn typecheck_imports<B>(
        &self,
        imports: impl IntoIterator<Item = &NamedExtern<B, T>>,
    ) -> anyhow::Result<ValidatedImports<B, T>>
    where
        B: Backend,
    {
        // 1, 2. ASSERT module is valid, done in Module construction

        // 3. Import count matches required imports
        let provided_imports: Vec<NamedExtern<B, T>> = imports
            .into_iter()
            .map(NamedExtern::<B, T>::clone)
            .collect_vec();
        let initializers = self.initializers();

        // Separate types
        // For now this is just imports
        let mut required_imports = Vec::new();
        for initializer in initializers {
            match initializer {
                Initializer::Import { name, field, index } => {
                    required_imports.push((name, field, index))
                }
            }
        }

        // 4. Match imports
        // First link on names
        let import_by_name: HashMap<(String, String), Extern<B, T>> = provided_imports
            .into_iter()
            .map(|ext| {
                let NamedExtern { module, name, ext } = ext;
                ((module.to_owned(), name.to_owned()), ext)
            })
            .collect();

        let mut validated_imports = ValidatedImports {
            functions: vec![],
            globals: vec![],
            tables: vec![],
            memories: vec![],
        };
        for (module_name, import_name, required_import) in required_imports {
            // 4.a. Check types are valid
            // Check name exists
            let provided_import: &Extern<B, T> = import_by_name
                .get(&(module_name, import_name))
                .ok_or(anyhow!("import with name {} not found", import_name))?;

            // Check provided import matches
            if provided_import.get_store_id() != self.id {
                anyhow!("imported extern is from a different store")?;
            }

            // 4.b. Check types match
            if !self.extern_is_type(&provided_import, required_import) {
                anyhow!(
                    "invalid import type: got {} but expected {:?}",
                    provided_import.type_name(),
                    required_type
                )?;
            }

            // Put in vec in order of import
            match provided_import.clone() {
                Extern::Func(fp) => validated_imports.functions.push(fp),
                Extern::Global(gp) => validated_imports.globals.push(gp),
                Extern::Table(tp) => validated_imports.tables.push(tp),
                Extern::Memory(mp) => validated_imports.memories.push(mp),
            };
        }

        return Ok(validated_imports);
    }
}
