pub mod error;
pub mod module_environ;

use crate::externs::{Extern, NamedExtern};
use crate::func::FuncAccessiblePtrs;
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
use anyhow::{anyhow, Context, Error};
use itertools::Itertools;
use std::borrow::Cow;
use std::collections::HashMap;
use std::slice::Iter;
use std::sync::Arc;
use wasm_spirv_funcgen::FuncAccessible;
use wasm_spirv_funcgen::{FuncData, FunctionModuleData};
use wasm_types::{FuncRef, Val, ValTypeByteCount};
use wasmparser::{Type, ValType, Validator};
use wgpu::BufferAsyncError;
use wgpu_async::async_queue::AsyncQueue;

/// A wasm module that has not been instantiated
pub struct Module {
    parsed: ParsedModuleUnit,
    name: String,
}

pub struct ValidatedImports {
    functions: Vec<UntypedFuncPtr>,
    globals: Vec<AbstractGlobalPtr>,
    tables: Vec<AbstractTablePtr>,
    memories: Vec<AbstractMemoryPtr>,
}

impl ValidatedImports {
    pub fn functions(&self) -> Iter<UntypedFuncPtr> {
        self.functions.iter()
    }
    pub fn globals(&self) -> Iter<AbstractGlobalPtr> {
        self.globals.iter()
    }
    pub fn tables(&self) -> Iter<AbstractTablePtr> {
        self.tables.iter()
    }
    pub fn memories(&self) -> Iter<AbstractMemoryPtr> {
        self.memories.iter()
    }
}

impl Module {
    fn parse(
        features: &wasmparser::WasmFeatures,
        wasm: Vec<u8>,
    ) -> Result<ParsedModuleUnit, Error> {
        let validator = Validator::new_with_features(features.clone());
        let parser = wasmparser::Parser::new(0);
        let parsed = ModuleEnviron::new(validator)
            .translate(parser, wasm)
            .context("failed to parse WebAssembly module")?;

        return Ok(parsed);
    }

    pub fn new<'a>(
        features: &wasmparser::WasmFeatures,
        bytes: impl IntoIterator<Item = &'a u8>,
        name: String,
    ) -> Result<Self, Error> {
        let wasm: Vec<_> = bytes.into_iter().map(|v| *v).collect();
        let wasm: Cow<'_, [u8]> = wat::parse_bytes(wasm.as_slice())?;
        let wasm = wasm.to_vec();

        let parsed = Self::parse(features, wasm)?;

        return Ok(Self { parsed, name });
    }

    /// See 4.5.4 of WASM spec 2.0
    /// Performs 1-4
    pub fn typecheck_imports(
        &self,
        provided_imports: &Vec<NamedExtern>,
    ) -> anyhow::Result<ValidatedImports> {
        // 1, 2. ASSERT module is valid, done in Module construction

        // 3. Import count matches required imports
        // Not required since we link on names instead

        // 4. Match imports
        // First link on names
        let import_by_name: HashMap<(String, String), Extern> = provided_imports
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
    pub(crate) async fn try_initialize_globals(
        &self,
        queue: &AsyncQueue,
        mutable_globals_instance: &mut MappedMutableGlobalsInstanceBuilder,
        immutable_globals_instance: &mut MappedImmutableGlobalsInstance,
        global_imports: impl Iterator<Item = AbstractGlobalPtr>,
        module_func_ptrs: &Vec<UntypedFuncPtr>,
    ) -> Result<Vec<AbstractGlobalPtr>, BufferAsyncError> {
        // Calculate space requirements
        let (immutables, mutables): (Vec<_>, Vec<_>) = self
            .parsed
            .borrow_sections()
            .globals
            .iter()
            .map(|g| (g.ty.mutable, g.ty.content_type.byte_count()))
            .partition(|(is_mutable, _)| *is_mutable);

        let is_immutable_mutable = immutables.first().unwrap_or(&(false, 0)).0;
        let is_mutable_mutable = mutables.first().unwrap_or(&(true, 0)).0;
        assert!(!is_immutable_mutable);
        assert!(is_mutable_mutable);

        let immutable_space: usize = immutables.into_iter().map(|(_, v)| usize::from(v)).sum();
        let mutable_space: usize = mutables.into_iter().map(|(_, v)| usize::from(v)).sum();

        // Reserve
        immutable_globals_instance.reserve(immutable_space);
        mutable_globals_instance.reserve(mutable_space);

        // Add the values
        let mut results = global_imports.into_iter().collect_vec();
        for global in self.parsed.borrow_sections().globals.iter() {
            let value = interpret_constexpr(
                queue,
                &global.initializer,
                mutable_globals_instance,
                immutable_globals_instance,
                &results,
                &module_func_ptrs,
            )
            .await?;
            let ptr: AbstractGlobalPtr = if global.ty.mutable {
                AbstractGlobalPtr::Mutable(mutable_globals_instance.try_push(queue, value).await?)
            } else {
                AbstractGlobalPtr::Immutable(
                    immutable_globals_instance.try_push(queue, value).await?,
                )
            };
            results.push(ptr);
        }

        return Ok(results);
    }

    /// Extends elements buffers to be shared by all stores of a set, as passive elements are immutable
    pub(crate) async fn try_initialize_elements(
        &self,
        queue: &AsyncQueue,
        elements: &mut MappedElementInstance,
        // Needed for const expr evaluation
        module_mutable_globals: &mut MappedMutableGlobalsInstanceBuilder,
        module_immutable_globals: &mut MappedImmutableGlobalsInstance,
        module_global_ptrs: &Vec<AbstractGlobalPtr>,
        module_func_ptrs: &Vec<UntypedFuncPtr>,
    ) -> Result<Vec<ElementPtr>, BufferAsyncError> {
        // Reserve space first
        let size: usize = std::mem::size_of::<FuncRef>()
            * self
                .parsed
                .borrow_sections()
                .elements
                .iter()
                .map(|e| e.items.len())
                .sum::<usize>();
        elements.reserve(size);

        // Then add
        let mut ptrs = Vec::new();
        for element in self.parsed.borrow_sections().elements.iter() {
            // Evaluate values
            let mut vals = Vec::new();
            for expr in element.items.iter() {
                let v = interpret_constexpr(
                    queue,
                    expr,
                    module_mutable_globals,
                    module_immutable_globals,
                    module_global_ptrs,
                    module_func_ptrs,
                )
                .await?;
                let v = match (v, &element.ty) {
                    (Val::FuncRef(fr), ValType::FuncRef) => fr.as_u32(),
                    (Val::ExternRef(er), ValType::ExternRef) => er.as_u32(),
                    _ => unreachable!(),
                };
                vals.push(v);
            }

            let ptr = elements
                .try_add_element(queue, element.ty.clone(), vals)
                .await?;
            ptrs.push(ptr);
        }

        return Ok(ptrs);
    }

    pub(crate) async fn try_initialize_tables<'a>(
        &'a self,
        queue: &AsyncQueue,
        tables: &mut MappedTableInstanceSetBuilder,
        imported_tables: impl IntoIterator<Item = &'a AbstractTablePtr>,
        elements: &mut MappedElementInstance,
        module_element_ptrs: &Vec<ElementPtr>,
        // Needed for const expr evaluation
        module_mutable_globals: &mut MappedMutableGlobalsInstanceBuilder,
        module_immutable_globals: &mut MappedImmutableGlobalsInstance,
        module_global_ptrs: &Vec<AbstractGlobalPtr>,
        module_func_ptrs: &Vec<UntypedFuncPtr>,
    ) -> Result<Vec<AbstractTablePtr>, BufferAsyncError> {
        // Pointers starts with imports
        let mut ptrs = imported_tables
            .into_iter()
            .map(|tp| tp.clone())
            .collect_vec();

        // Create tables first
        for table_plan in self.parsed.borrow_sections().tables.iter() {
            let ptr = tables.add_table(table_plan);
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
                        queue,
                        offset_expr,
                        module_mutable_globals,
                        module_immutable_globals,
                        module_global_ptrs,
                        module_func_ptrs,
                    )
                    .await?;
                    let offset = match v {
                        Val::I32(v) => v as usize,
                        Val::I64(v) => v as usize,
                        _ => unreachable!(),
                    };

                    let data = elements.try_get(queue, element_ptr).await?;

                    tables
                        .try_initialize(queue, table_ptr, &data, offset)
                        .await?;

                    // Then we can drop this element
                    elements.drop(element_ptr).await;
                }
                _ => {}
            }
        }

        return Ok(ptrs);
    }

    pub(crate) async fn try_initialize_datas(
        &self,
        queue: &AsyncQueue,
        datas: &mut MappedDataInstance,
    ) -> Result<Vec<DataPtr>, BufferAsyncError> {
        // Reserve space first
        let size: usize = self
            .parsed
            .borrow_sections()
            .datas
            .iter()
            .map(|e| e.data.len())
            .sum();
        datas.reserve(size);

        // Then add
        let mut ptrs = Vec::new();
        for data in self.parsed.borrow_sections().datas.iter() {
            let ptr = datas.try_add_data(queue, data.data).await?;
            ptrs.push(ptr);
        }

        return Ok(ptrs);
    }

    pub(crate) async fn try_initialize_memories<'a>(
        &'a self,
        queue: &AsyncQueue,
        memory_set: &mut MappedMemoryInstanceSetBuilder,
        imported_memories: impl IntoIterator<Item = &'a AbstractMemoryPtr>,
        datas: &mut MappedDataInstance,
        module_data_ptrs: &Vec<DataPtr>,
        // Needed for const expr evaluation
        module_mutable_globals: &mut MappedMutableGlobalsInstanceBuilder,
        module_immutable_globals: &mut MappedImmutableGlobalsInstance,
        module_global_ptrs: &Vec<AbstractGlobalPtr>,
        module_func_ptrs: &Vec<UntypedFuncPtr>,
    ) -> Result<Vec<AbstractMemoryPtr>, BufferAsyncError> {
        // Pointers starts with imports
        let mut ptrs = imported_memories
            .into_iter()
            .map(AbstractMemoryPtr::clone)
            .collect_vec();

        // Create memories first
        for memory_type in self.parsed.borrow_sections().memories.iter() {
            let ptr = memory_set.add_memory(memory_type);
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
                        queue,
                        offset_expr,
                        module_mutable_globals,
                        module_immutable_globals,
                        module_global_ptrs,
                        module_func_ptrs,
                    )
                    .await?;
                    let offset = match v {
                        Val::I32(v) => v as usize,
                        Val::I64(v) => v as usize,
                        _ => unreachable!(),
                    };

                    let data = datas.try_get(queue, data_ptr).await?;

                    memory_set
                        .try_initialize(queue, memory_ptr, &data, offset)
                        .await?;

                    // Then we can drop this data
                    datas.drop(data_ptr).await;
                }
                _ => {}
            }
        }

        return Ok(ptrs);
    }

    pub(crate) fn try_initialize_function_definitions<'a>(
        &'a self,
        functions: &mut FuncsInstance,
        func_imports: impl IntoIterator<Item = &'a UntypedFuncPtr>,
    ) -> anyhow::Result<Vec<UntypedFuncPtr>> {
        let mut ptrs = func_imports
            .into_iter()
            .map(UntypedFuncPtr::clone)
            .collect_vec();

        let sections = self.parsed.borrow_sections();

        let module_data = Arc::new(FunctionModuleData {
            types: sections.types.clone(),
        });

        functions.reserve(sections.functions.len());
        let mut new_ptrs = sections
            .functions
            .iter()
            .map(|func| {
                match sections
                    .types
                    .get(
                        usize::try_from(func.type_id)
                            .expect("module cannot reside in memory unless #items <= |word|"),
                    )
                    .unwrap()
                    .clone()
                {
                    Type::Func(ty) => FuncData {
                        ty,
                        locals: func.locals.clone(),
                        operators: func.operators.clone(),
                        module_data: Arc::clone(&module_data),
                    },
                }
            })
            .map(|data| functions.register_definition(data))
            .collect_vec();

        ptrs.append(&mut new_ptrs);

        return Ok(ptrs);
    }

    /// Checks that the types of the given pointers match the expected pointers of the imports + definitions of this module.
    fn debug_typecheck_func_ptrs(&self, func_ptrs: &Vec<UntypedFuncPtr>) {
        let sections = self.parsed.borrow_sections();

        let required_imported_functions = sections
            .imports
            .iter()
            .filter_map(|(_, _, import_type)| match import_type {
                ImportTypeRef::Func(f_ty_id) => Some(
                    match &sections.types
                        [usize::try_from(*f_ty_id).expect("16 bit architectures are unsupported")]
                    {
                        Type::Func(f_ty) => f_ty.clone(),
                    },
                ),
                _ => None,
            })
            .collect_vec();
        let mut required_defined_functions = sections
            .functions
            .iter()
            .map(|func| {
                match &sections.types
                    [usize::try_from(func.type_id).expect("16 bit architectures are unsupported")]
                {
                    Type::Func(f_ty) => f_ty.clone(),
                }
            })
            .collect_vec();

        let mut required_functions = required_imported_functions;
        required_functions.append(&mut required_defined_functions);

        let ptr_types = func_ptrs.iter().map(|ptr| ptr.ty().clone()).collect_vec();
        debug_assert_eq!(
            required_functions, ptr_types,
            "function pointer types did not match required function types"
        );
    }

    /// Checks that the types of the given pointers match the expected pointers of the imports + definitions of this module.
    fn debug_typecheck_global_ptrs(&self, global_ptrs: &Vec<AbstractGlobalPtr>) {
        let sections = self.parsed.borrow_sections();

        let required_imported_globals = sections
            .imports
            .iter()
            .filter_map(|(_, _, import_type)| match import_type {
                ImportTypeRef::Global(g_ty) => Some(g_ty.clone()),
                _ => None,
            })
            .collect_vec();
        let mut required_defined_globals = sections
            .globals
            .iter()
            .map(|global| global.ty)
            .collect_vec();

        let mut required_globals = required_imported_globals;
        required_globals.append(&mut required_defined_globals);

        let ptr_types = global_ptrs.iter().map(|ptr| ptr.ty().clone()).collect_vec();
        debug_assert_eq!(
            required_globals, ptr_types,
            "global pointer types did not match required global types"
        );
    }

    /// Checks that the types of the given pointers match the expected pointers of the imports + definitions of this module.
    fn debug_typecheck_element_ptrs(&self, element_ptrs: &Vec<ElementPtr>) {
        let sections = self.parsed.borrow_sections();

        let required_elements = sections
            .elements
            .iter()
            .map(|element| element.ty)
            .collect_vec();

        let ptr_types = element_ptrs
            .iter()
            .map(|ptr| ptr.ty().clone())
            .collect_vec();
        debug_assert_eq!(
            required_elements, ptr_types,
            "element pointer types did not match required element types"
        );
    }

    /// Checks that the types of the given pointers match the expected pointers of the imports + definitions of this module.
    fn debug_typecheck_table_ptrs(&self, table_ptrs: &Vec<AbstractTablePtr>) {
        let sections = self.parsed.borrow_sections();

        let required_imported_tables = sections
            .imports
            .iter()
            .filter_map(|(_, _, import_type)| match import_type {
                ImportTypeRef::Table(t_ty) => Some(t_ty.clone()),
                _ => None,
            })
            .collect_vec();
        let mut required_defined_tables = sections.tables.clone();

        let mut required_tables = required_imported_tables;
        required_tables.append(&mut required_defined_tables);

        let ptr_types = table_ptrs.iter().map(|ptr| ptr.ty().clone()).collect_vec();
        debug_assert_eq!(
            required_tables, ptr_types,
            "table pointer types did not match required table types"
        );
    }

    /// Checks that the types of the given pointers match the expected pointers of the imports + definitions of this module.
    fn debug_typecheck_data_ptrs(&self, data_ptrs: &Vec<DataPtr>) {
        let sections = self.parsed.borrow_sections();

        debug_assert_eq!(
            sections.datas.len(),
            data_ptrs.len(),
            "insufficient data pointers defined"
        );
    }

    /// Checks that the types of the given pointers match the expected pointers of the imports + definitions of this module.
    fn debug_typecheck_memory_ptrs(&self, memory_ptrs: &Vec<AbstractMemoryPtr>) {
        let sections = self.parsed.borrow_sections();

        let required_imported_memories = sections
            .imports
            .iter()
            .filter_map(|(_, _, import_type)| match import_type {
                ImportTypeRef::Memory(m_ty) => Some(m_ty.clone()),
                _ => None,
            })
            .collect_vec();
        let mut required_defined_memories = sections.memories.clone();

        let mut required_memories = required_imported_memories;
        required_memories.append(&mut required_defined_memories);

        let ptr_types = memory_ptrs.iter().map(|ptr| ptr.ty().clone()).collect_vec();
        debug_assert_eq!(
            required_memories, ptr_types,
            "memory pointer types did not match required memory types"
        );
    }

    /// Takes everything accessable by this module and resolve all function body references
    pub(crate) fn try_initialize_function_bodies<'a>(
        &'a self,
        functions: &mut FuncsInstance,
        accessible: &FuncAccessiblePtrs,
    ) -> anyhow::Result<()> {
        // Check that the data we've been given for the initialisation of this module makes sense.
        // This is a debug check to ensure everything has been implemented properly by us. This method
        // should not be callable from outside of the crate, so this is more to check our invariants than
        // input validation.
        self.debug_typecheck_func_ptrs(&accessible.func_index_lookup);
        self.debug_typecheck_global_ptrs(&accessible.global_index_lookup);
        self.debug_typecheck_element_ptrs(&accessible.element_index_lookup);
        self.debug_typecheck_table_ptrs(&accessible.table_index_lookup);
        self.debug_typecheck_data_ptrs(&accessible.data_index_lookup);
        self.debug_typecheck_memory_ptrs(&accessible.memory_index_lookup);

        // Link import data
        let defined_func_count = self.parsed.borrow_sections().functions.len();
        let defined_func_start = accessible.func_index_lookup.len() - defined_func_count;
        let defined_function_ptrs = &accessible.func_index_lookup[defined_func_start..];
        debug_assert_eq!(defined_function_ptrs.len(), defined_func_count);

        let function_accessibles = Arc::new(accessible.to_indices());

        for ptr in defined_function_ptrs {
            functions.link_function_imports(ptr, Arc::clone(&function_accessibles));
        }

        return Ok(());
    }

    pub fn start_fn(&self, module_func_ptrs: &Vec<UntypedFuncPtr>) -> Option<UntypedFuncPtr> {
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
