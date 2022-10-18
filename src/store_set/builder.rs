use crate::externs::NamedExtern;
use crate::instance::data::DataInstance;
use crate::instance::element::ElementInstance;
use crate::instance::func::{FuncsInstance, UntypedFuncPtr};
use crate::instance::global::abstr::AbstractGlobalInstance;
use crate::instance::memory::abstr::AbstractMemoryInstanceSet;
use crate::instance::table::abstr::AbstractTableInstanceSet;
use crate::instance::ModuleInstance;
use crate::{Backend, Engine, Func, Module, StoreSet};
use std::sync::Arc;

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
}

impl<B, T> StoreSetBuilder<B, T>
where
    B: Backend,
{
    pub fn new(engine: &Engine<B>) -> Self {
        Self {
            backend: engine.backend(),

            functions: FuncsInstance::new(),
            tables: AbstractTableInstanceSet::new(engine.backend()),
            memories: AbstractMemoryInstanceSet::new(engine.backend()),
            globals: AbstractGlobalInstance::new(engine.backend()),
            elements: ElementInstance::new(engine.backend()),
            datas: DataInstance::new(engine.backend()),
        }
    }

    pub fn backend(&self) -> Arc<B> {
        self.backend.clone()
    }

    /// Instantiation within a builder moves all of the data to the device. This means that constructing
    /// stores from the builder involves no copying of data from the CPU to the GPU, only within the GPU.
    pub async fn instantiate_module(
        &mut self,
        module: &Module<'_, B>,
        imports: Vec<NamedExtern<B, T>>,
    ) -> anyhow::Result<ModuleInstance<B, T>> {
        // Validation
        let validated_imports = module.typecheck_imports(&imports)?;

        // Globals
        let global_ptrs = module
            .initialize_globals(&mut self.globals, validated_imports.globals())
            .await;

        // Predict the function pointers that we *will* be creating
        let predicted_func_ptrs = module.predict_functions(&self.functions);

        // Elements
        let element_ptrs = module
            .initialize_elements(
                &mut self.elements,
                &mut self.globals,
                &global_ptrs,
                &predicted_func_ptrs,
            )
            .await;

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
            .await;

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
            .await;

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
            .await;
        assert_eq!(predicted_func_ptrs, func_ptrs);

        // Final setup, consisting of the Start function, must be performed in the build step if it
        // calls any host functions
        let start_fn = module.start_fn(&func_ptrs);

        // Collect exports from pointers
        let exports = module.collect_exports(&global_ptrs, &table_ptrs, &memory_ptrs, &func_ptrs);

        let funcs = func_ptrs.into_iter().collect();
        let tables = table_ptrs.into_iter().collect();
        let memories = memory_ptrs.into_iter().collect();
        let globals = global_ptrs.into_iter().collect();
        return Ok(ModuleInstance::new(
            funcs, tables, memories, globals, exports, start_fn,
        ));
    }

    pub fn register_function(&mut self, func: Func<B, T>) -> UntypedFuncPtr<B, T> {
        return self.functions.register(func);
    }

    /// Takes this builder and makes it immutable, allowing instances to be created from it
    pub fn complete(self) -> CompletedBuilder<B, T> {
        CompletedBuilder { inner: self }
    }
}

pub struct CompletedBuilder<B: Backend, T> {
    inner: StoreSetBuilder<B, T>, // Just hide the builder :)
}

impl<B: Backend, T> CompletedBuilder<B, T> {
    /// Takes the instructions provided to this builder and produces a collection of stores which can
    /// be used to evaluate instructions
    pub async fn build(&self, values: impl IntoIterator<Item = T>) -> StoreSet<B, T> {
        // Here we take all of the initialisation that we did that can be shared and spin it into several
        // instances. This shouldn't involve moving any data to the device, instead data that has already
        // been provided to the device should be cloned and specialised as needed for a collection of instances
        unimplemented!()
    }
}
