pub mod error;
pub mod module_environ;

use crate::externs::{Extern, NamedExtern};
use crate::instance::data::{DataPtr, HostDataInstance};
use crate::instance::element::{ElementPtr, HostElementInstance};
use crate::instance::func::{FuncsInstance, UntypedFuncPtr};
use crate::instance::global::abstr::{AbstractGlobalPtr, HostAbstractGlobalInstance};
use crate::instance::memory::abstr::{AbstractMemoryPtr, HostAbstractMemoryInstanceSet};
use crate::instance::table::abstr::{AbstractTablePtr, HostAbstractTableInstanceSet};
use crate::module::module_environ::{
    ImportTypeRef, ModuleEnviron, ModuleExport, ParsedElementKind, ParsedModuleUnit,
};
use crate::typed::{wasm_ty_bytes, FuncRef, Val};
use crate::{Backend, Engine};
use anyhow::{anyhow, Context, Error};
use itertools::Itertools;
use std::borrow::Cow;
use std::collections::HashMap;
use std::slice::Iter;
use std::sync::Arc;
use wasmparser::{Type, ValType, Validator};

/// A wasm module that has not been instantiated
pub struct Module<B>
where
    B: Backend,
{
    name: String,
    backend: Arc<B>,
    parsed: ParsedModuleUnit,
}

pub struct ValidatedImports<B, T>
where
    B: Backend,
{
    functions: Vec<UntypedFuncPtr<B, T>>,
    globals: Vec<AbstractGlobalPtr<B, T>>,
    tables: Vec<AbstractTablePtr<B, T>>,
    memories: Vec<AbstractMemoryPtr<B, T>>,
}

impl<B, T> ValidatedImports<B, T>
where
    B: Backend,
{
    pub fn functions(&self) -> Iter<UntypedFuncPtr<B, T>> {
        self.functions.iter()
    }
    pub fn globals(&self) -> Iter<AbstractGlobalPtr<B, T>> {
        self.globals.iter()
    }
    pub fn tables(&self) -> Iter<AbstractTablePtr<B, T>> {
        self.tables.iter()
    }
    pub fn memories(&self) -> Iter<AbstractMemoryPtr<B, T>> {
        self.memories.iter()
    }
}

impl<B> Module<B>
where
    B: Backend,
{
    fn parse(engine: &Engine<B>, wasm: Vec<u8>) -> Result<ParsedModuleUnit, Error> {
        let mut validator = Validator::new_with_features(engine.config().features.clone());
        let parser = wasmparser::Parser::new(0);
        let parsed = ModuleEnviron::new(validator)
            .translate(parser, wasm)
            .context("failed to parse WebAssembly module")?;

        return Ok(parsed);
    }

    pub fn new<'a>(
        engine: &Engine<B>,
        bytes: impl IntoIterator<Item = &'a u8>,
        name: &str,
    ) -> Result<Self, Error> {
        let wasm: Vec<_> = bytes.into_iter().map(|v| *v).collect();
        let wasm: Cow<'_, [u8]> = wat::parse_bytes(wasm.as_slice())?;
        let wasm = wasm.to_vec();

        let parsed = Self::parse(engine, wasm)?;

        return Ok(Self {
            name: name.to_owned(),
            backend: engine.backend(),
            parsed,
        });
    }

    /// See 4.5.4 of WASM spec 2.0
    /// Performs 1-4
    pub fn typecheck_imports<T>(
        &self,
        provided_imports: &Vec<NamedExtern<B, T>>,
    ) -> anyhow::Result<ValidatedImports<B, T>> {
        // 1, 2. ASSERT module is valid, done in Module construction

        // 3. Import count matches required imports
        // Not required since we link on names instead

        // 4. Match imports
        // First link on names
        let import_by_name: HashMap<(String, String), Extern<B, T>> = provided_imports
            .into_iter()
            .map(|ext| {
                let NamedExtern { module, name, ext } = ext;
                ((module.to_owned(), name.to_owned()), ext.clone())
            })
            .collect();

        let mut validated_imports = ValidatedImports {
            functions: vec![],
            globals: vec![],
            tables: vec![],
            memories: vec![],
        };
        for (module, name, required_import) in self.parsed.borrow_sections().imports.iter() {
            // Get provided
            let key = (module.to_string(), name.to_string());
            let provided_import = import_by_name.get(&key).ok_or(anyhow!(
                "missing import with module {} and name {}",
                module,
                name
            ))?;

            // Check type
            let matches = match (required_import, provided_import) {
                (ImportTypeRef::Func(f_id), Extern::Func(f2)) => {
                    let ty = self
                        .parsed
                        .borrow_sections()
                        .types
                        .get((*f_id) as usize)
                        .expect("import function id was out of range");
                    match ty {
                        Type::Func(f1) => f2.ty().eq(f1),
                        _ => false,
                    }
                }
                (ImportTypeRef::Table(t1), Extern::Table(t2)) => t2.is_type(t1),
                (ImportTypeRef::Memory(m1), Extern::Memory(m2)) => m2.is_type(m1),
                (ImportTypeRef::Global(g1), Extern::Global(g2)) => g2.is_type(g1),
                _ => false,
            };

            if !matches {
                return Err(anyhow!(
                    "import types do not match - expected {:?} but got {:?}",
                    required_import,
                    provided_import.signature()
                ));
            } else {
                // Add to validated
                match provided_import {
                    Extern::Func(f) => validated_imports.functions.push(f.clone()),
                    Extern::Global(g) => validated_imports.globals.push(g.clone()),
                    Extern::Table(t) => validated_imports.tables.push(t.clone()),
                    Extern::Memory(m) => validated_imports.memories.push(m.clone()),
                }
            }
        }

        return Ok(validated_imports);
    }

    /// Extends a globals memory buffer and indirection buffer to fit the globals contained in this
    /// module, then writes the initial values
    pub(crate) async fn initialize_globals<T>(
        &self,
        globals_instance: &mut HostAbstractGlobalInstance<B>,
        global_imports: impl Iterator<Item = AbstractGlobalPtr<B, T>>,
        module_func_ptrs: &Vec<UntypedFuncPtr<B, T>>,
    ) -> Vec<AbstractGlobalPtr<B, T>> {
        // Calculate space requirements
        let (immutables, mutables): (Vec<_>, Vec<_>) = self
            .parsed
            .borrow_sections()
            .globals
            .iter()
            .map(|g| (g.ty.mutable, wasm_ty_bytes(g.ty.content_type)))
            .partition(|(is_mutable, _)| *is_mutable);

        let is_immutable = immutables.first().unwrap_or(&(false, 0)).0;
        let is_mutable = mutables.first().unwrap_or(&(true, 0)).0;
        assert!(is_immutable);
        assert!(is_mutable);

        let immutable_space: usize = immutables.into_iter().map(|(_, v)| v).sum();
        let mutable_space: usize = mutables.into_iter().map(|(_, v)| v).sum();

        // Reserve
        globals_instance.reserve_immutable(immutable_space).await;
        globals_instance.reserve_mutable(mutable_space).await;

        // Add the values
        let mut results = global_imports.into_iter().collect_vec();
        for global in self.parsed.borrow_sections().globals.iter() {
            let ptr: AbstractGlobalPtr<B, T> = globals_instance
                .add_global(global.clone(), &results, &module_func_ptrs)
                .await;
            results.push(ptr);
        }

        return results;
    }

    pub(crate) fn predict_functions<T>(
        &self,
        functions: &FuncsInstance<B, T>,
    ) -> Vec<UntypedFuncPtr<B, T>> {
        let types = self.parsed.borrow_sections().functions.iter().map(|f| {
            self.parsed
                .borrow_sections()
                .types
                .get(f.type_id as usize)
                .expect("function type index out of range")
        });
        functions.predict(types)
    }

    /// Extends elements buffers to be shared by all stores of a set, as passive elements are immutable
    pub(crate) async fn initialize_elements<T>(
        &self,
        elements: &mut HostElementInstance<B>,
        // Needed for const expr evaluation
        module_globals: &mut HostAbstractGlobalInstance<B>,
        module_global_ptrs: &Vec<AbstractGlobalPtr<B, T>>,
        module_func_ptrs: &Vec<UntypedFuncPtr<B, T>>,
    ) -> Vec<ElementPtr<B, T>> {
        // Reserve space first
        let size: usize = std::mem::size_of::<FuncRef>()
            * self
                .parsed
                .borrow_sections()
                .elements
                .iter()
                .map(|e| e.items.len())
                .sum::<usize>();
        elements.reserve(size).await;

        // Then add
        let mut ptrs = Vec::new();
        for element in self.parsed.borrow_sections().elements.iter() {
            // Evaluate values
            let mut vals = Vec::new();
            for expr in element.items.iter() {
                let v = module_globals
                    .interpret_constexpr(expr, module_global_ptrs, module_func_ptrs)
                    .await;
                let v = match (v, &element.ty) {
                    (Val::FuncRef(fr), ValType::FuncRef) => fr.as_u32(),
                    (Val::ExternRef(er), ValType::ExternRef) => er.as_u32(),
                    _ => unreachable!(),
                };
                vals.push(v);
            }

            let ptr = elements.add_element(vals).await;
            ptrs.push(ptr);
        }

        return ptrs;
    }

    pub(crate) async fn initialize_tables<T>(
        &self,
        tables: &mut HostAbstractTableInstanceSet<B>,
        imported_tables: Iter<'_, AbstractTablePtr<B, T>>,
        elements: &mut HostElementInstance<B>,
        module_element_ptrs: &Vec<ElementPtr<B, T>>,
        // Needed for const expr evaluation
        module_globals: &mut HostAbstractGlobalInstance<B>,
        module_global_ptrs: &Vec<AbstractGlobalPtr<B, T>>,
        module_func_ptrs: &Vec<UntypedFuncPtr<B, T>>,
    ) -> Vec<AbstractTablePtr<B, T>> {
        // Pointers starts with imports
        let mut ptrs = imported_tables.map(|tp| tp.clone()).collect_vec();

        // Create tables first
        for table_plan in self.parsed.borrow_sections().tables.iter() {
            let ptr = tables.add_table(table_plan).await;
            ptrs.push(ptr);
        }

        // Initialise from elements
        for (element, element_ptr) in self
            .parsed
            .borrow_sections()
            .elements
            .iter()
            .zip_eq(module_element_ptrs)
        {
            match &element.kind {
                ParsedElementKind::Active {
                    table_index,
                    offset_expr,
                } => {
                    let table_ptr = ptrs
                        .get((*table_index) as usize)
                        .expect("table index out of range");
                    let v = module_globals
                        .interpret_constexpr(offset_expr, module_global_ptrs, module_func_ptrs)
                        .await;
                    let offset = match v {
                        Val::I32(v) => v as usize,
                        Val::I64(v) => v as usize,
                        _ => unreachable!(),
                    };

                    let data = elements.get(element_ptr).await;

                    tables.initialize(table_ptr, data, offset).await
                }
                _ => {}
            }
        }

        return ptrs;
    }

    pub(crate) async fn initialize_datas<T>(
        &self,
        datas: &mut HostDataInstance<B>,
    ) -> Vec<DataPtr<B, T>> {
        // Reserve space first
        let size: usize = self
            .parsed
            .borrow_sections()
            .datas
            .iter()
            .map(|e| e.data.len())
            .sum();
        datas.reserve(size).await;

        // Then add
        let mut ptrs = Vec::new();
        for data in self.parsed.borrow_sections().datas.iter() {
            let ptr = datas.add_data(data.data).await;
            ptrs.push(ptr);
        }

        return ptrs;
    }

    pub(crate) async fn initialize_memories<T>(
        &self,
        memory_set: &mut HostAbstractMemoryInstanceSet<B>,
        imported_memories: Iter<'_, AbstractMemoryPtr<B, T>>,
        datas: &mut HostDataInstance<B>,
        module_data_ptrs: &Vec<DataPtr<B, T>>,
        // Needed for const expr evaluation
        module_globals: &mut HostAbstractGlobalInstance<B>,
        module_global_ptrs: &Vec<AbstractGlobalPtr<B, T>>,
        module_func_ptrs: &Vec<UntypedFuncPtr<B, T>>,
    ) -> Vec<AbstractMemoryPtr<B, T>> {
        unimplemented!()
    }

    pub(crate) async fn initialize_functions<T>(
        &self,
        functions: &mut FuncsInstance<B, T>,
        func_imports: Iter<'_, UntypedFuncPtr<B, T>>,
        module_globals: &Vec<AbstractGlobalPtr<B, T>>,
        module_elements: &Vec<ElementPtr<B, T>>,
        module_tables: &Vec<AbstractTablePtr<B, T>>,
        module_datas: &Vec<DataPtr<B, T>>,
        module_memories: &Vec<AbstractMemoryPtr<B, T>>,
    ) -> Vec<UntypedFuncPtr<B, T>> {
        unimplemented!()
    }

    pub fn start_fn<T>(
        &self,
        module_func_ptrs: &Vec<UntypedFuncPtr<B, T>>,
    ) -> Option<UntypedFuncPtr<B, T>> {
        match self.parsed.borrow_sections().start_func {
            None => None,
            Some(i) => {
                let i = usize::try_from(i).unwrap();
                let ptr = module_func_ptrs.get(i).expect("function referenced was outside of module - this should have been caught at module validation time");
                return Some(ptr.clone());
            }
        }
    }

    pub fn exports(&self) -> &HashMap<String, ModuleExport> {
        &self.parsed.borrow_sections().exports
    }
}
