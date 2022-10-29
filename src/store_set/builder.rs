use crate::externs::NamedExtern;
use crate::instance::data::{DeviceDataInstance, HostDataInstance};
use crate::instance::element::{DeviceElementInstance, HostElementInstance};
use crate::instance::func::{FuncsInstance, UntypedFuncPtr};
use crate::instance::global::abstr::{DeviceAbstractGlobalInstance, HostAbstractGlobalInstance};
use crate::instance::memory::abstr::{
    DeviceAbstractMemoryInstanceSet, HostAbstractMemoryInstanceSet,
};
use crate::instance::table::abstr::{DeviceAbstractTableInstanceSet, HostAbstractTableInstanceSet};
use crate::instance::ModuleInstanceSet;
use crate::store_set::DeviceStoreSetData;
use crate::{Backend, DeviceStoreSet, Engine, Func, Module};
use std::future::join;
use std::sync::Arc;

/// Acts like a traditional OOP factory where we initialise modules into this before
/// creating single Stores after all initialization is done, to amortize the instantiation cost
pub struct StoreSetBuilder<B, T>
where
    B: Backend,
{
    backend: Arc<B>,

    tables: HostAbstractTableInstanceSet<B>,
    memories: HostAbstractMemoryInstanceSet<B>,
    globals: HostAbstractGlobalInstance<B>,
    // Immutable so don't need to be abstr
    elements: HostElementInstance<B>,
    datas: HostDataInstance<B>,
    functions: FuncsInstance<B, T>,
}

impl<B, T> StoreSetBuilder<B, T>
where
    B: Backend,
{
    pub async fn new(engine: &Engine<B>) -> Self {
        let backend = engine.backend();

        let globals_fut = HostAbstractGlobalInstance::new(backend.as_ref());
        let elements_fut = DeviceElementInstance::new(backend.as_ref()).map();
        let datas_fut = DeviceDataInstance::new(backend.as_ref()).map();

        let (globals, elements, datas) = join!(globals_fut, elements_fut, datas_fut).await;

        Self {
            functions: FuncsInstance::new(),
            tables: HostAbstractTableInstanceSet::new(engine.backend()),
            memories: HostAbstractMemoryInstanceSet::new(engine.backend()),
            globals,
            elements,
            datas,
            backend,
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
        imports: Vec<NamedExtern<B, T>>,
    ) -> anyhow::Result<ModuleInstanceSet<B, T>> {
        // Validation
        let validated_imports = module.typecheck_imports(&imports)?;

        // Predict the function pointers that we *will* be creating, for ref evaluation
        let predicted_func_ptrs = module.predict_functions(&self.functions);

        // Globals
        let global_ptrs = module
            .initialize_globals(
                &mut self.globals,
                validated_imports.globals().map(|p| p.clone()),
                &predicted_func_ptrs,
            )
            .await;

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
        let data_ptrs = module.initialize_datas(&mut self.datas).await;

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
        if predicted_func_ptrs != func_ptrs {
            panic!("predicted function pointers did not match later calculated pointers");
        }

        // Final setup, consisting of the Start function, must be performed in the build step if it
        // calls any host functions
        let start_fn = module.start_fn(&func_ptrs);

        // Lock vectors to be immutable
        let func_ptrs = func_ptrs.into_iter().collect();
        let table_ptrs = table_ptrs.into_iter().collect();
        let memory_ptrs = memory_ptrs.into_iter().collect();
        let global_ptrs = global_ptrs.into_iter().collect();
        let exports = module
            .exports()
            .into_iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        return Ok(ModuleInstanceSet::new(
            func_ptrs,
            table_ptrs,
            memory_ptrs,
            global_ptrs,
            exports,
            start_fn,
        ));
    }

    pub fn register_function(&mut self, func: Func<B, T>) -> UntypedFuncPtr<B, T> {
        return self.functions.register(func);
    }

    /// Takes this builder and makes it immutable, allowing instances to be created from it
    pub async fn complete(self) -> CompletedBuilder<B, T> {
        let Self {
            backend,
            tables,
            memories,
            globals,
            elements,
            datas,
            functions,
        } = self;

        let globals = globals.unmap().await;
        let elements = elements.unmap().await;
        let datas = datas.unmap().await;
        let tables = tables.unmap().await;
        let memories = memories.unmap().await;
        CompletedBuilder {
            backend,
            tables,
            memories,
            globals,
            elements: Arc::new(elements),
            datas: Arc::new(datas),
            functions: Arc::new(functions),
        }
    }
}

pub struct CompletedBuilder<B: Backend, T> {
    backend: Arc<B>,

    // Move host things to GPU
    tables: DeviceAbstractTableInstanceSet<B>,
    memories: DeviceAbstractMemoryInstanceSet<B>,
    globals: DeviceAbstractGlobalInstance<B>,
    elements: Arc<DeviceElementInstance<B>>,
    datas: Arc<DeviceDataInstance<B>>,
    functions: Arc<FuncsInstance<B, T>>,
}

impl<B: Backend, T> CompletedBuilder<B, T> {
    /// Takes the instructions provided to this builder and produces a collection of stores which can
    /// be used to evaluate instructions
    pub async fn build(&self, values: impl IntoIterator<Item = T>) -> DeviceStoreSet<B, T> {
        // Here we take all of the initialisation that we did that can be shared and spin it into several
        // instances. This shouldn't involve moving any data to the device, instead data that has already
        // been provided to the device should be cloned and specialised as needed for a collection of instances
        let data: Vec<_> = values.into_iter().collect();

        let tables = self.tables.build(data.len()).await;

        let memories = self.memories.build(data.len()).await;

        let (mutable_globals, immutable_globals) =
            self.globals.build(self.backend.clone(), data.len()).await;

        DeviceStoreSet {
            backend: self.backend.clone(),
            data,
            functions: self.functions.clone(),
            elements: self.elements.clone(),
            datas: self.datas.clone(),
            immutable_globals,
            owned: DeviceStoreSetData {
                tables,
                memories,
                mutable_globals,
            },
        }
    }
}
