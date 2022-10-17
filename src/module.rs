pub mod error;
pub mod module_environ;

use crate::externs::NamedExtern;
use crate::instance::data::{DataInstance, DataPtr};
use crate::instance::element::{ElementInstance, ElementPtr};
use crate::instance::func::{FuncsInstance, UntypedFuncPtr};
use crate::instance::global::abstr::{AbstractGlobalInstance, AbstractGlobalPtr};
use crate::instance::memory::abstr::{AbstractMemoryInstanceSet, AbstractMemoryPtr};
use crate::instance::table::abstr::{AbstractTableInstanceSet, AbstractTablePtr};
use crate::module::module_environ::{
    ImportTypeRef, ModuleEnviron, ParsedElementItems, ParsedElementKind, ParsedModule,
};
use crate::typed::{wasm_ty_bytes, FuncRef, Val};
use crate::{Backend, Engine, Extern};
use anyhow::{anyhow, Context, Error};
use itertools::Itertools;
use std::borrow::Cow;
use std::collections::HashMap;
use std::slice::Iter;
use std::sync::Arc;
use wasmparser::{Type, ValType, Validator};

/// A wasm module that has not been instantiated
pub struct Module<'a, B>
where
    B: Backend,
{
    backend: Arc<B>,
    source: Vec<u8>,
    parsed: ParsedModule<'a>,
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

impl<'a, B> Module<'a, B>
where
    B: Backend,
{
    pub fn new(engine: &Engine<B>, bytes: Vec<u8>) -> Result<Self, Error> {
        let wasm: Cow<'a, [u8]> = wat::parse_bytes(bytes.as_slice())?;

        let mut validator = Validator::new_with_features(engine.config().features.clone());
        let parser = wasmparser::Parser::new(0);
        let parsed = ModuleEnviron::new(validator)
            .translate(parser, &wasm)
            .context("failed to parse WebAssembly module")?;

        return Ok(Self {
            backend: engine.backend(),
            source: bytes,
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
                ((module.to_owned(), name.to_owned()), ext)
            })
            .collect();

        let mut validated_imports = ValidatedImports {
            functions: vec![],
            globals: vec![],
            tables: vec![],
            memories: vec![],
        };
        for (module, name, required_import) in self.parsed.imports.iter() {
            // Get provided
            let provided_import = import_by_name
                .get(&(module.to_owned(), name.to_owned()))
                .ok_or(anyhow!(
                    "missing import with module {} and name {}",
                    module,
                    name
                ))?;

            // Check type
            let matches = match required_import {
                ImportTypeRef::Func(f_id) => {
                    let ty = self
                        .parsed
                        .types
                        .get((*f_id) as usize)
                        .expect("import function id was out of range");
                    match (ty, provided_import) {
                        (Type::Func(f1), Extern::Func(f2)) => f2.is_type(f1),
                        _ => false,
                    }
                }
                ImportTypeRef::Table(t1) => match provided_import {
                    Extern::Table(t2) => t2.is_type(t1),
                    _ => false,
                },
                ImportTypeRef::Memory(m1) => match provided_import {
                    Extern::Memory(m2) => m2.is_type(m1),
                    _ => false,
                },
                ImportTypeRef::Global(g1) => match provided_import {
                    Extern::Global(g2) => g2.is_type(g1),
                    _ => false,
                },
            };

            if !matches {
                return Err(anyhow!(
                    "import types do not match - expected {:?} but got {:?}",
                    required_import,
                    provided_import
                ));
            }
        }

        return Ok(validated_imports);
    }

    /// Extends a globals memory buffer and indirection buffer to fit the globals contained in this
    /// module, then writes the initial values
    pub(crate) async fn initialize_globals<T>(
        &self,
        globals_instance: &mut AbstractGlobalInstance<B>,
        globals: impl Iterator<Item = AbstractGlobalPtr<B, T>>,
    ) -> anyhow::Result<Vec<AbstractGlobalPtr<B, T>>> {
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
        globals_instance.reserve(values_len).await;

        // Add the values
        let mut imports_iter = globals.into_iter();
        let mut results = Vec::new();
        for global in self.parsed.globals.iter() {
            let ptr: AbstractGlobalPtr<B, T> = globals_instance
                .add_global(global.clone(), &mut imports_iter, results.as_slice())
                .await?;
            results.push(ptr);
        }

        return Ok(results);
    }

    pub(crate) fn predict_functions<T>(
        &self,
        functions: &FuncsInstance<B, T>,
    ) -> Vec<UntypedFuncPtr<B, T>> {
        let types = self.parsed.functions.iter().map(|f| {
            self.parsed
                .types
                .get(f.type_id as usize)
                .expect("function type index out of range")
        });
        functions.predict(types)
    }

    /// Extends elements buffers to be shared by all stores of a set, as passive elements are immutable
    pub(crate) async fn initialize_elements<T>(
        &self,
        elements: &mut ElementInstance<B>,
        // Needed for const expr evaluation
        globals: &mut AbstractGlobalInstance<B>,
        global_ptrs: &Vec<AbstractGlobalPtr<B, T>>,
        func_ptrs: &Vec<UntypedFuncPtr<B, T>>,
    ) -> anyhow::Result<Vec<ElementPtr<B, T>>> {
        // Reserve space first
        let size: usize = std::mem::size_of::<FuncRef>()
            * self.parsed.elements.iter().map(|e| e.items.len()).sum();
        elements.reserve(size).await;

        // Then add
        let mut ptrs = Vec::new();
        for element in self.parsed.elements.iter() {
            // Evaluate values
            let vals = match &element.items {
                ParsedElementItems::Func(vs) => vs,
                ParsedElementItems::Expr(exprs) => {
                    let mut vs = Vec::new();
                    for expr in exprs.iter() {
                        let v = globals
                            .interpret_constexpr(expr, global_ptrs, func_ptrs)
                            .await;
                        let v = match (v, &element.ty) {
                            (Val::FuncRef(fr), ValType::FuncRef) => fr.as_u32(),
                            (Val::ExternRef(er), ValType::ExternRef) => er.as_u32(),
                            _ => unreachable!(),
                        };
                        vs.push(v);
                    }

                    &vs
                }
            };

            let ptr = elements.add_element(vals).await?;
            ptrs.push(ptr);
        }

        return Ok(ptrs);
    }

    pub(crate) async fn initialize_tables<T>(
        &self,
        tables: &mut AbstractTableInstanceSet<B>,
        imported_tables: Iter<'_, AbstractTablePtr<B, T>>,
        elements: &mut ElementInstance<B>,
        module_element_ptrs: &Vec<ElementPtr<B, T>>,
        // Needed for const expr evaluation
        globals: &mut AbstractGlobalInstance<B>,
        global_ptrs: &Vec<AbstractGlobalPtr<B, T>>,
        func_ptrs: &Vec<UntypedFuncPtr<B, T>>,
    ) -> anyhow::Result<Vec<AbstractTablePtr<B, T>>> {
        // Pointers starts with imports
        let mut ptrs = Vec::from(imported_tables.map(|tp| tp.clone()));

        // Create tables first
        for table_plan in self.parsed.tables.iter() {
            let ptr = tables.add_table(table_plan).await;
            ptrs.push(ptr);
        }

        // Initialise from elements
        for (element, element_ptr) in self.parsed.elements.iter().zip_eq(module_element_ptrs) {
            match &element.kind {
                ParsedElementKind::Active {
                    table_index,
                    offset_expr,
                } => {
                    let table_ptr = ptrs
                        .get((*table_index) as usize)
                        .expect("table index out of range");
                    let v = globals
                        .interpret_constexpr(offset_expr, global_ptrs, func_ptrs)
                        .await;
                    let offset = match v {
                        Val::I32(v) => v as usize,
                        Val::I64(v) => v as usize,
                        _ => unreachable!(),
                    };

                    let data = elements.get(element_ptr).await?;

                    tables.initialize(table_ptr, data, offset)
                }
                _ => {}
            }
        }

        return Ok(ptrs);
    }

    pub(crate) async fn initialize_datas<T>(
        &self,
        datas: &mut DataInstance<B>,
    ) -> anyhow::Result<Vec<DataPtr<B, T>>> {
        // Reserve space first
        let size: usize = self.parsed.datas.iter().map(|e| e.data.len()).sum();
        datas.reserve(size).await;

        // Then add
        let mut ptrs = Vec::new();
        for data in self.parsed.datas.iter() {
            let ptr = datas.add_data(data.data).await?;
            ptrs.push(ptr);
        }

        return Ok(ptrs);
    }

    pub(crate) async fn initialize_memories<T>(
        &self,
        memory_set: &mut AbstractMemoryInstanceSet<B>,
        imported_memories: Iter<'_, AbstractMemoryPtr<B, T>>,
        datas: &mut DataInstance<B>,
        module_data_ptrs: &Vec<DataPtr<B, T>>,
        // Needed for const expr evaluation
        globals: &mut AbstractGlobalInstance<B>,
        global_ptrs: &Vec<AbstractGlobalPtr<B, T>>,
        func_ptrs: &Vec<UntypedFuncPtr<B, T>>,
    ) -> anyhow::Result<Vec<AbstractMemoryPtr<B, T>>> {
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
    ) -> anyhow::Result<Vec<UntypedFuncPtr<B, T>>> {
        unimplemented!()
    }
}
