pub mod error;
pub mod module_environ;

use crate::externs::{Extern, NamedExtern};
use crate::instance::data::{DataPtr, MappedDataInstance};
use crate::instance::element::{ElementPtr, MappedElementInstance};
use crate::instance::func::{FuncsInstance, UntypedFuncPtr};
use crate::instance::global::builder::{AbstractGlobalPtr, MappedMutableGlobalsInstanceBuilder};
use crate::instance::global::immutable::MappedImmutableGlobalsInstance;
use crate::instance::memory::builder::{AbstractMemoryPtr, MappedMemoryInstanceSetBuilder};
use crate::instance::table::builder::{AbstractTablePtr, MappedTableInstanceSetBuilder};
use crate::module::module_environ::{
    ImportTypeRef, ModuleEnviron, ModuleExport, ParsedDataKind, ParsedElementKind, ParsedModuleUnit,
};
use crate::store_set::builder::interpret_constexpr;
use crate::typed::{wasm_ty_bytes, FuncRef, Val};
use crate::Engine;
use anyhow::{anyhow, Context, Error};
use itertools::Itertools;
use std::borrow::Cow;
use std::collections::HashMap;
use std::slice::Iter;
use wasmparser::{Type, ValType, Validator};

/// A wasm module that has not been instantiated
pub struct Module {
    parsed: ParsedModuleUnit,
}

pub struct ValidatedImports<T> {
    functions: Vec<UntypedFuncPtr<T>>,
    globals: Vec<AbstractGlobalPtr<T>>,
    tables: Vec<AbstractTablePtr<T>>,
    memories: Vec<AbstractMemoryPtr<T>>,
}

impl<T> ValidatedImports<T> {
    pub fn functions(&self) -> Iter<UntypedFuncPtr<T>> {
        self.functions.iter()
    }
    pub fn globals(&self) -> Iter<AbstractGlobalPtr<T>> {
        self.globals.iter()
    }
    pub fn tables(&self) -> Iter<AbstractTablePtr<T>> {
        self.tables.iter()
    }
    pub fn memories(&self) -> Iter<AbstractMemoryPtr<T>> {
        self.memories.iter()
    }
}

impl Module {
    fn parse(engine: &Engine, wasm: Vec<u8>) -> Result<ParsedModuleUnit, Error> {
        let mut validator = Validator::new_with_features(engine.config().features.clone());
        let parser = wasmparser::Parser::new(0);
        let parsed = ModuleEnviron::new(validator)
            .translate(parser, wasm)
            .context("failed to parse WebAssembly module")?;

        return Ok(parsed);
    }

    pub fn new<'a>(
        engine: &Engine,
        bytes: impl IntoIterator<Item = &'a u8>,
    ) -> Result<Self, Error> {
        let wasm: Vec<_> = bytes.into_iter().map(|v| *v).collect();
        let wasm: Cow<'_, [u8]> = wat::parse_bytes(wasm.as_slice())?;
        let wasm = wasm.to_vec();

        let parsed = Self::parse(engine, wasm)?;

        return Ok(Self { parsed });
    }

    /// See 4.5.4 of WASM spec 2.0
    /// Performs 1-4
    pub fn typecheck_imports<T>(
        &self,
        provided_imports: &Vec<NamedExtern<T>>,
    ) -> anyhow::Result<ValidatedImports<T>> {
        // 1, 2. ASSERT module is valid, done in Module construction

        // 3. Import count matches required imports
        // Not required since we link on names instead

        // 4. Match imports
        // First link on names
        let import_by_name: HashMap<(String, String), Extern<T>> = provided_imports
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
        mutable_globals_instance: &mut MappedMutableGlobalsInstanceBuilder,
        immutable_globals_instance: &mut MappedImmutableGlobalsInstance,
        global_imports: impl Iterator<Item = AbstractGlobalPtr<T>>,
        module_func_ptrs: &Vec<UntypedFuncPtr<T>>,
    ) -> Vec<AbstractGlobalPtr<T>> {
        // Calculate space requirements
        let (immutables, mutables): (Vec<_>, Vec<_>) = self
            .parsed
            .borrow_sections()
            .globals
            .iter()
            .map(|g| (g.ty.mutable, wasm_ty_bytes(g.ty.content_type)))
            .partition(|(is_mutable, _)| *is_mutable);

        let is_immutable_mutable = immutables.first().unwrap_or(&(false, 0)).0;
        let is_mutable_mutable = mutables.first().unwrap_or(&(true, 0)).0;
        assert!(!is_immutable_mutable);
        assert!(is_mutable_mutable);

        let immutable_space: usize = immutables.into_iter().map(|(_, v)| v).sum();
        let mutable_space: usize = mutables.into_iter().map(|(_, v)| v).sum();

        // Reserve
        immutable_globals_instance.reserve(immutable_space).await;
        mutable_globals_instance.reserve(mutable_space).await;

        // Add the values
        let mut results = global_imports.into_iter().collect_vec();
        for global in self.parsed.borrow_sections().globals.iter() {
            let value = interpret_constexpr(
                &global.initializer,
                mutable_globals_instance,
                immutable_globals_instance,
                &results,
                &module_func_ptrs,
            )
            .await;
            let ptr: AbstractGlobalPtr<T> = if global.ty.mutable {
                AbstractGlobalPtr::Mutable(mutable_globals_instance.push(value).await)
            } else {
                AbstractGlobalPtr::Immutable(immutable_globals_instance.push(value).await)
            };
            results.push(ptr);
        }

        return results;
    }

    pub(crate) fn predict_functions<T>(
        &self,
        functions: &FuncsInstance<T>,
    ) -> Vec<UntypedFuncPtr<T>> {
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
        elements: &mut MappedElementInstance,
        // Needed for const expr evaluation
        module_mutable_globals: &mut MappedMutableGlobalsInstanceBuilder,
        module_immutable_globals: &mut MappedImmutableGlobalsInstance,
        module_global_ptrs: &Vec<AbstractGlobalPtr<T>>,
        module_func_ptrs: &Vec<UntypedFuncPtr<T>>,
    ) -> Vec<ElementPtr<T>> {
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
                let v = interpret_constexpr(
                    expr,
                    module_mutable_globals,
                    module_immutable_globals,
                    module_global_ptrs,
                    module_func_ptrs,
                )
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
        tables: &mut MappedTableInstanceSetBuilder,
        imported_tables: Iter<'_, AbstractTablePtr<T>>,
        elements: &mut MappedElementInstance,
        module_element_ptrs: &Vec<ElementPtr<T>>,
        // Needed for const expr evaluation
        module_mutable_globals: &mut MappedMutableGlobalsInstanceBuilder,
        module_immutable_globals: &mut MappedImmutableGlobalsInstance,
        module_global_ptrs: &Vec<AbstractGlobalPtr<T>>,
        module_func_ptrs: &Vec<UntypedFuncPtr<T>>,
    ) -> Vec<AbstractTablePtr<T>> {
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
                    let v = interpret_constexpr(
                        offset_expr,
                        module_mutable_globals,
                        module_immutable_globals,
                        module_global_ptrs,
                        module_func_ptrs,
                    )
                    .await;
                    let offset = match v {
                        Val::I32(v) => v as usize,
                        Val::I64(v) => v as usize,
                        _ => unreachable!(),
                    };

                    let data = elements.get(element_ptr).await;

                    tables.initialize(table_ptr, data, offset).await;

                    // Then we can drop this element
                    elements.drop(element_ptr).await;
                }
                _ => {}
            }
        }

        return ptrs;
    }

    pub(crate) async fn initialize_datas<T>(
        &self,
        datas: &mut MappedDataInstance,
    ) -> Vec<DataPtr<T>> {
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
        memory_set: &mut MappedMemoryInstanceSetBuilder,
        imported_memories: Iter<'_, AbstractMemoryPtr<T>>,
        datas: &mut MappedDataInstance,
        module_data_ptrs: &Vec<DataPtr<T>>,
        // Needed for const expr evaluation
        module_mutable_globals: &mut MappedMutableGlobalsInstanceBuilder,
        module_immutable_globals: &mut MappedImmutableGlobalsInstance,
        module_global_ptrs: &Vec<AbstractGlobalPtr<T>>,
        module_func_ptrs: &Vec<UntypedFuncPtr<T>>,
    ) -> Vec<AbstractMemoryPtr<T>> {
        // Pointers starts with imports
        let mut ptrs = imported_memories.map(|tp| tp.clone()).collect_vec();

        // Create memories first
        for memory_type in self.parsed.borrow_sections().memories.iter() {
            let ptr = memory_set.add_memory(memory_type).await;
            ptrs.push(ptr);
        }

        // Initialise from datas
        for (data, data_ptr) in self
            .parsed
            .borrow_sections()
            .datas
            .iter()
            .zip_eq(module_data_ptrs)
        {
            match &data.kind {
                ParsedDataKind::Active {
                    memory_index,
                    offset_expr,
                } => {
                    assert_eq!(*memory_index, 0);

                    let memory_ptr = ptrs
                        .get((*memory_index) as usize)
                        .expect("memory index out of range");
                    let v = interpret_constexpr(
                        offset_expr,
                        module_mutable_globals,
                        module_immutable_globals,
                        module_global_ptrs,
                        module_func_ptrs,
                    )
                    .await;
                    let offset = match v {
                        Val::I32(v) => v as usize,
                        Val::I64(v) => v as usize,
                        _ => unreachable!(),
                    };

                    let data = datas.get(data_ptr).await;

                    memory_set.initialize(memory_ptr, data, offset).await;

                    // Then we can drop this data
                    datas.drop(data_ptr).await;
                }
                _ => {}
            }
        }

        return ptrs;
    }

    pub(crate) async fn initialize_functions<T>(
        &self,
        functions: &mut FuncsInstance<T>,
        func_imports: Iter<'_, UntypedFuncPtr<T>>,
        module_globals: &Vec<AbstractGlobalPtr<T>>,
        module_elements: &Vec<ElementPtr<T>>,
        module_tables: &Vec<AbstractTablePtr<T>>,
        module_datas: &Vec<DataPtr<T>>,
        module_memories: &Vec<AbstractMemoryPtr<T>>,
    ) -> Vec<UntypedFuncPtr<T>> {
        if self.parsed.borrow_sections().functions.is_empty() {
            return vec![];
        }

        let ptrs: Vec<UntypedFuncPtr<T>> = func_imports.collect_vec();

        let sections = self.parsed.borrow_sections();
        for func in sections.functions {
            let ty = match sections.types.get(func.type_id).unwrap().clone() {
                Type::FuncType(ty) => ty,
            };

            // TODO: Compile and add functions
        }

        return ptrs;
    }

    pub fn start_fn<T>(
        &self,
        module_func_ptrs: &Vec<UntypedFuncPtr<T>>,
    ) -> Option<UntypedFuncPtr<T>> {
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
