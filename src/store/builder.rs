use crate::atomic_counter::AtomicCounter;
use crate::externs::NamedExtern;
use crate::instance::abstr::global::AbstractGlobalInstance;
use crate::instance::abstr::memory::AbstractMemoryInstanceSet;
use crate::instance::abstr::table::AbstractTableInstanceSet;
use crate::instance::data::DataInstance;
use crate::instance::element::ElementInstance;
use crate::instance::func::{AbstractUntypedFuncPtr, FuncsInstance};
use crate::instance::ModuleInstance;
use crate::{Backend, Engine, Func, Module, StoreSet};
use futures::StreamExt;
use std::hash::Hash;
use std::sync::Arc;

static STORE_SET_COUNTER: AtomicCounter = AtomicCounter::new(); // Use as store hash & equality

/// Acts like a traditional OOP factory where we initialise modules into this before
/// creating single Stores after all initialization is done, to amortize the instantiation cost
pub struct StoreSetBuilder<B, T>
where
    B: Backend,
{
    backend: Arc<B>,

    tables: AbstractTableInstanceSet<B>,
    memories: AbstractMemoryInstanceSet<B>,
    globals: AbstractGlobalInstance<B>,
    // Immutable so don't need to be abstr
    elements: ElementInstance<B>,
    datas: DataInstance<B>,
    functions: FuncsInstance<B, T>,

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

            functions: FuncsInstance::new(engine.backend(), id),
            tables: AbstractTableInstanceSet::new(engine.backend(), id),
            memories: AbstractMemoryInstanceSet::new(engine.backend(), id),
            globals: AbstractGlobalInstance::new(engine.backend(), id),
            elements: ElementInstance::new(engine.backend(), id),
            datas: DataInstance::new(engine.backend(), id),

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
            .initialize_memories(
                &mut self.memories,
                validated_imports.memories(),
                &mut self.datas,
                &data_ptrs,
                &mut self.globals,
                &global_ptrs,
                &predicted_func_ptrs,
            )
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

    pub fn register_function(&mut self, func: Func<B, T>) -> AbstractUntypedFuncPtr<B, T> {
        return self.functions.register(func);
    }

    /// Takes the instructions provided to this builder and produces a collection of stores which can
    /// be used to evaluate instructions
    ///
    /// Must consume self so that pointers are valid
    pub async fn build(self, values: impl IntoIterator<Item = T>) -> StoreSet<B, T> {
        // Here we take all of the initialisation that we did that can be shared and spin it into several
        // instances. This shouldn't involve moving any data to the device, instead data that has already
        // been provided to the device should be cloned and specialised as needed for a collection of instances
        unimplemented!()
    }
}
