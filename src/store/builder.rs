use crate::atomic_counter::AtomicCounter;
use crate::externs::NamedExtern;
use crate::func::TypedFuncPtr;
use crate::instance::element::ElementInstance;
use crate::instance::global::{GlobalInstance, GlobalPtr, GlobalType};
use crate::instance::table::{TableInstance, TableInstanceSet};
use crate::instance::ModuleInstance;
use crate::memory::{DynamicMemoryBlock, Memory};
use crate::module::module_environ::Global;
use crate::read_only::{AppendOnlyVec, ReadOnly};
use crate::store::ptrs::{FuncPtr, MemoryPtr, StorePtr};
use crate::typed::{ExternRef, FuncRef, Val, WasmTyVec};
use crate::{Backend, Engine, Extern, Func, Module, StoreSet};
use anyhow::{anyhow, Context};
use elsa::{FrozenMap, FrozenVec};
use futures::StreamExt;
use itertools::Itertools;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::intrinsics::unreachable;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use wasmparser::{Operator, ValType};
use wasmtime::{FuncType, ValType};
use wasmtime_environ::{
    EntityIndex, FunctionType, Global, Initializer, MemoryPlan, TablePlan, WasmFuncType, WasmType,
};

static STORE_SET_COUNTER: AtomicCounter = AtomicCounter::new(); // Use as store hash & equality

/// View like a 'set' of WASM state machines which we perform SIMD instantiation etc on.
/// In reality acts more like a traditional OOP factory where we initialise modules into this before
/// creating single Stores after all initialization is done, to amortize the instantiation cost
pub struct StoreSetBuilder<B, T>
where
    B: Backend,
{
    backend: Arc<B>,

    functions: Vec<Func<B, T>>,
    tables: TableInstanceSet<B>,
    memories: Vec<DynamicMemoryBlock<B>>,
    globals: GlobalInstance<B>,
    elements: ElementInstance<B>,
    datas: Vec<DynamicMemoryBlock<B>>,

    id: usize,
}

impl<B, T> StoreSetBuilder<B, T>
where
    B: Backend,
{
    pub fn new(engine: &Engine<B>) -> Self {
        let id = STORE_SET_COUNTER.next();
        Self {
            backend: engine.backend(),

            functions: Vec::new(),
            tables: TableInstanceSet::new(),
            memories: Vec::new(),
            globals: GlobalInstance::new(engine.backend(), id),
            elements: ElementInstance::new(engine.backend(), id),
            datas: Vec::new(),

            id,
        }
    }

    pub fn backend(&self) -> Arc<B> {
        self.backend.clone()
    }

    /// Instantiation within a builder moves all of the data to the device. This means that constructing
    /// stores from the builder involves no copying of data from the CPU to the GPU, only within the GPU.
    pub async fn instantiate_module(
        &mut self,
        module: &Module<B>,
        imports: impl IntoIterator<Item = NamedExtern<B, T>>,
    ) -> anyhow::Result<ModuleInstance<B, T>> {
        // Validation
        let validated_imports = module.typecheck_imports(imports)?;

        // Globals
        let global_ptrs = module
            .initialize_globals(&mut self.globals, validated_imports.globals())
            .await?;

        // Predict the function pointers that we *will* be creating
        let predicted_func_ptrs = module.predict_functions(self.id, self.functions.len());

        // Elements
        let element_ptrs = module
            .initialize_elements(
                &mut self.elements,
                &mut self.globals,
                &global_ptrs,
                &predicted_func_ptrs,
            )
            .await?;

        // Tables
        let table_ptrs = module
            .initialize_tables(
                &mut self.tables,
                validated_imports.tables(),
                &mut self.elements,
                &element_ptrs,
                &mut self.globals,
                &global_ptrs,
                &predicted_func_ptrs,
            )
            .await?;

        // Datas
        let data_ptrs = module.initialize_datas(&mut self.datas).await?;

        // Memories
        let memory_ptrs = module
            .initialize_memories(&mut self.memories, validated_imports.memories(), &data_ptrs)
            .await?;

        // Functions - they take everything
        let func_ptrs = module
            .initialize_functions(
                &mut self.functions,
                validated_imports.functions(),
                &global_ptrs,
                &element_ptrs,
                &table_ptrs,
                &data_ptrs,
                &memory_ptrs,
            )
            .await?;
        debug_assert_eq!(predicted_func_ptrs, func_ptrs);

        // Final setup, consisting of the Start function, must be performed in the build step if it
        // calls any host functions
        if let Some(_start) = module.start_fn_index() {
            unimplemented!()
        }

        // Collect exports from pointers
        let exports =
            module.collect_exports(&global_ptrs, &table_ptrs, &memory_ptrs, &func_ptrs)?;

        let funcs = func_ptrs.into_values().collect();
        let tables = table_ptrs.into_values().collect();
        let memories = memory_ptrs.into_values().collect();
        let globals = global_ptrs.into_values().collect();
        return Ok(ModuleInstance::new(
            self.id, funcs, tables, memories, globals, exports,
        ));
    }

    pub fn register_function(&self, func: Func<B, T>) -> FuncPtr<B, T> {
        let ty = func.ty();

        let ptr = self.functions.push_get_index(func);

        return FuncPtr::new(ptr, self.id, ty);
    }

    /// Takes the instructions provided to this builder and produces a collection of stores which can
    /// be used to evaluate instructions
    ///
    /// Must consume self so that pointers are valid
    pub async fn build(self, values: impl IntoIterator<Item = T>) -> StoreSet<B, T> {
        // Here we take all of the initialisation that we did that can be shared and spin it into several
        // instances. This shouldn't involve moving any data to the device, instead data that has already
        // been provided to the device should be cloned and specialised as needed for a collection of instances
    }
}
