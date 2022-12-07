use crate::externs::NamedExtern;
use crate::instance::data::{MappedDataInstance, UnmappedDataInstance};
use crate::instance::element::{MappedElementInstance, UnmappedElementInstance};
use crate::instance::func::{FuncsInstance, UntypedFuncPtr};
use crate::instance::global::builder::{
    MappedGlobalInstanceBuilder, UnmappedGlobalInstanceBuilder,
};
use crate::instance::memory::builder::{
    MappedMemoryInstanceSetBuilder, UnmappedMemoryInstanceSetBuilder,
};
use crate::instance::table::builder::{
    MappedTableInstanceSetBuilder, UnmappedTableInstanceSetBuilder,
};
use crate::instance::ModuleInstanceSet;
use crate::store_set::DeviceStoreSetData;
use crate::{DeviceStoreSet, Engine, Func, Module};
use lf_hal::backend::Backend;
use std::future::join;
use std::sync::Arc;

/// Acts like a traditional OOP factory where we initialise modules into this before
/// creating single Stores after all initialization is done, to amortize the instantiation cost
pub struct StoreSetBuilder<B, T>
where
    B: Backend,
{
    backend: Arc<B>,

    tables: MappedTableInstanceSetBuilder<B>,
    memories: MappedMemoryInstanceSetBuilder<B>,
    globals: MappedGlobalInstanceBuilder<B>,
    // Immutable so don't need to be abstr
    elements: MappedElementInstance<B>,
    datas: MappedDataInstance<B>,
    functions: FuncsInstance<B, T>,
}

impl<B, T> StoreSetBuilder<B, T>
where
    B: Backend,
{
    pub async fn new(engine: &Engine<B>) -> Self {
        let backend = engine.backend();

        let globals_fut = MappedGlobalInstanceBuilder::new(backend.as_ref());
        let elements_fut = MappedElementInstance::new(backend.as_ref());
        let datas_fut = MappedDataInstance::new(backend.as_ref());

        let (globals, elements, datas) = join!(globals_fut, elements_fut, datas_fut).await;

        Self {
            functions: FuncsInstance::new(),
            tables: MappedTableInstanceSetBuilder::new(engine.backend()),
            memories: MappedMemoryInstanceSetBuilder::new(engine.backend()),
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
    tables: UnmappedTableInstanceSetBuilder<B>,
    memories: UnmappedMemoryInstanceSetBuilder<B>,
    globals: UnmappedGlobalInstanceBuilder<B>,
    elements: Arc<UnmappedElementInstance<B>>,
    datas: Arc<UnmappedDataInstance<B>>,
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

#[cfg(test)]
mod tests {
    use crate::tests_lib::{gen_test_memory_string, get_backend};
    use crate::{block_test, imports, wasp, Config, StoreSetBuilder};
    use anyhow::anyhow;
    use std::sync::Arc;
    macro_rules! data_tests {
        ($($value:expr),* $(,)?) => {
        $(
            block_test!($value, test_data_buffer_populated_correctly);
        )*
        };
    }

    data_tests!(0, 1, 7, 8, 9, 1023, 1024, 1025, 4095, 4096, 4097);

    #[inline(never)]
    async fn test_data_buffer_populated_correctly(size: usize) {
        let backend = get_backend().await;

        let (expected_data, data_str) = gen_test_memory_string(size, 84637322u32);

        let engine = wasp::Engine::new(backend, Config::default());

        let mut stores_builder = StoreSetBuilder::<_, ()>::new(&engine).await;

        let wat = format!(
            r#"
            (module
                (data "{}")
            )
        "#,
            data_str
        );
        let wat = wat.into_bytes();
        let module = wasp::Module::new(&engine, &wat, "testmod1").unwrap();

        let _instance = stores_builder
            .instantiate_module(&module, imports! {})
            .await
            .expect("could not instantiate all modules");

        let set = stores_builder.complete().await;

        let buffers = Arc::try_unwrap(set.datas)
            .map_err(|_| {
                anyhow!("multiple references existed to buffer that should probably be owned")
            })
            .unwrap();

        assert_eq!(buffers.read_all().await, expected_data)
    }
}
