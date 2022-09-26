pub mod error;
pub mod module_environ;

use crate::externs::NamedExtern;
use crate::instance::element::{ElementInstance, ElementPtr};
use crate::instance::global::{GlobalInstance, GlobalPtr};
use crate::instance::table::{TableInstance, TableInstanceSet, TablePtr};
use crate::module::module_environ::ModuleExport::Table;
use crate::module::module_environ::{
    Global, ImportTypeRef, ModuleEnviron, ParsedElementItems, ParsedElementKind, ParsedModule,
};
use crate::store::ptrs::{ElementPtr, FuncPtr, MemoryPtr};
use crate::typed::{wasm_ty_bytes, FuncRef, Val};
use crate::{Backend, Engine, Extern};
use anyhow::{anyhow, Context, Error};
use itertools::Itertools;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::intrinsics::{size_of, unreachable};
use std::ops::Index;
use std::slice::Iter;
use std::sync::Arc;
use wasmparser::types::{EntityType, TypeId};
use wasmparser::{ElementKind, Type, ValType, Validator};

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
        globals_instance: &mut GlobalInstance<B>,
        globals: impl Iterator<Item = GlobalPtr<B, T>>,
    ) -> anyhow::Result<Vec<GlobalPtr<B, T>>> {
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
        for (global) in globals.into_iter() {
            let ptr: GlobalPtr<B, T> = globals_instance
                .add_global(global, &mut imports_iter, results.as_slice())
                .await?;
            results.push(ptr);
        }

        return Ok(results);
    }

    pub(crate) fn predict_functions<T>(&self, id: usize, start: usize) -> Vec<FuncPtr<B, T>> {
        self.parsed
            .functions
            .iter()
            .enumerate()
            .map(|(i, f)| {
                FuncPtr::new(
                    start + i,
                    id,
                    match self
                        .parsed
                        .types
                        .get(f.type_id as usize)
                        .expect("function type index out of range")
                    {
                        Type::Func(f) => f.clone(),
                    },
                )
            })
            .collect_vec()
    }

    /// Extends elements buffers to be shared by all stores of a set, as passive elements are immutable
    pub(crate) async fn initialize_elements<T>(
        &self,
        elements: &mut ElementInstance<B>,
        // Needed for const expr evaluation
        globals: &mut GlobalInstance<B>,
        global_ptrs: &Vec<GlobalPtr<B, T>>,
        func_ptrs: &Vec<FuncPtr<B, T>>,
    ) -> anyhow::Result<Vec<ElementPtr<B, T>>> {
        // Reserve space first
        let size: usize =
            size_of::<FuncRef>() * self.parsed.elements.iter().map(|e| e.range.len()).sum();
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
        tables: &mut TableInstanceSet<B>,
        imported_tables: Iter<TablePtr<B, T>>,
        elements: &mut ElementInstance<B>,
        module_element_ptrs: &Vec<ElementPtr<B, T>>,
        // Needed for const expr evaluation
        globals: &mut GlobalInstance<B>,
        global_ptrs: &Vec<GlobalPtr<B, T>>,
        func_ptrs: &Vec<FuncPtr<B, T>>,
    ) -> Vec<TablePtr<B, T>> {
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

        return ptrs;
    }
}
