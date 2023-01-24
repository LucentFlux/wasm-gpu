use crate::externs::NamedExtern;
use crate::func::assembled_module::AssembledModule;
use crate::instance::data::{MappedDataInstance, UnmappedDataInstance};
use crate::instance::element::{MappedElementInstance, UnmappedElementInstance};
use crate::instance::func::{FuncsInstance, UntypedFuncPtr};
use crate::instance::global::builder::{
    AbstractGlobalPtr, MappedMutableGlobalsInstanceBuilder, UnmappedMutableGlobalsInstanceBuilder,
};
use crate::instance::global::immutable::{
    MappedImmutableGlobalsInstance, UnmappedImmutableGlobalsInstance,
};
use crate::instance::memory::builder::{
    MappedMemoryInstanceSetBuilder, UnmappedMemoryInstanceSetBuilder,
};
use crate::instance::table::builder::{
    MappedTableInstanceSetBuilder, UnmappedTableInstanceSetBuilder,
};
use crate::instance::ModuleInstanceReferences;
use crate::store_set::UnmappedStoreSetData;
use crate::{DeviceStoreSet, Engine, ExternRef, Func, FuncRef, Ieee32, Ieee64, Module, Val};
use perfect_derive::perfect_derive;
use std::sync::Arc;
use wasmparser::{Operator, ValType};
use wgpu::BufferAsyncError;
use wgpu_async::async_device::OutOfMemoryError;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::{DelayedOutOfMemoryError, LazilyUnmappable, MemorySystem};
use wgpu_lazybuffers_macros::lazy_mappable;

/// Used during instantiation to evaluate an expression in a single pass
pub(crate) async fn interpret_constexpr<'data, T>(
    queue: &AsyncQueue,
    constr_expr: &Vec<Operator<'data>>,
    mutable_globals: &mut MappedMutableGlobalsInstanceBuilder,
    immutable_globals: &mut MappedImmutableGlobalsInstance,
    global_ptrs: &Vec<AbstractGlobalPtr<T>>,
    func_ptrs: &Vec<UntypedFuncPtr<T>>,
) -> Result<Val, BufferAsyncError> {
    let mut stack = Vec::new();

    let mut iter = constr_expr.into_iter();
    while let Some(expr) = iter.next() {
        match expr {
            Operator::I32Const { value } => stack.push(Val::I32(*value)),
            Operator::I64Const { value } => stack.push(Val::I64(*value)),
            Operator::F32Const { value } => stack.push(Val::F32(Ieee32::from(*value))),
            Operator::F64Const { value } => stack.push(Val::F64(Ieee64::from(*value))),
            Operator::V128Const { value } => {
                stack.push(Val::V128(u128::from_le_bytes(value.bytes().clone())))
            }
            Operator::RefNull { ty } => match ty {
                ValType::FuncRef => stack.push(Val::FuncRef(FuncRef::none())),
                ValType::ExternRef => stack.push(Val::ExternRef(ExternRef::none())),
                _ => unreachable!(),
            },
            Operator::RefFunc { function_index } => {
                let function_index = usize::try_from(*function_index).unwrap();
                let function_ptr = func_ptrs
                    .get(function_index)
                    .expect("function index out of range of module functions");
                stack.push(Val::FuncRef(function_ptr.to_func_ref()))
            }
            Operator::GlobalGet { global_index } => {
                let global_index = usize::try_from(*global_index).unwrap();
                let global_ptr = global_ptrs
                    .get(global_index)
                    .expect("global index out of range of module globals");
                let global_val = match global_ptr {
                    AbstractGlobalPtr::Immutable(imm_ptr) => {
                        immutable_globals.try_get(queue, imm_ptr).await
                    }
                    AbstractGlobalPtr::Mutable(mut_ptr) => {
                        mutable_globals.try_get(queue, mut_ptr).await
                    }
                }?;

                stack.push(global_val)
            }
            Operator::End => {
                if !iter.next().is_none() {
                    // End at end
                    panic!("end expression was found before the end of the constexpr - this should be caught by validation earlier")
                }
                break;
            }
            _ => unreachable!(),
        }
    }

    let res = stack.pop().expect("expression did not result in a value");

    return Ok(res);
}

/// Acts like a traditional OOP factory where we initialise modules into this before
/// creating single Stores after all initialization is done, to amortize the instantiation cost
#[lazy_mappable(MappedStoreSetBuilder)]
#[perfect_derive(Debug)]
pub struct UnmappedStoreSetBuilder<T> {
    #[map(MappedTableInstanceSetBuilder)]
    tables: UnmappedTableInstanceSetBuilder,
    #[map(MappedMemoryInstanceSetBuilder)]
    memories: UnmappedMemoryInstanceSetBuilder,
    #[map(MappedMutableGlobalsInstanceBuilder)]
    mutable_globals: UnmappedMutableGlobalsInstanceBuilder,
    #[map(MappedElementInstance)]
    elements: UnmappedElementInstance,
    #[map(MappedDataInstance)]
    datas: UnmappedDataInstance,
    #[map(MappedImmutableGlobalsInstance)]
    immutable_globals: UnmappedImmutableGlobalsInstance,

    functions: FuncsInstance<T>,
}

impl<T> MappedStoreSetBuilder<T> {
    pub fn new(memory_system: &MemorySystem) -> Self {
        Self {
            functions: FuncsInstance::new(),
            tables: MappedTableInstanceSetBuilder::new(memory_system),
            memories: MappedMemoryInstanceSetBuilder::new(memory_system),
            immutable_globals: MappedImmutableGlobalsInstance::new(memory_system),
            mutable_globals: MappedMutableGlobalsInstanceBuilder::new(memory_system),
            elements: MappedElementInstance::new(memory_system),
            datas: MappedDataInstance::new(memory_system),
        }
    }

    pub(crate) async fn snapshot(src: &DeviceStoreSet<T>, store_index: usize) -> Self {
        // We're doing this so you can execute a bit then load more modules, then execute some more.
        // See wizer for the idea origin.
        todo!()
    }

    /// Instantiation within a builder moves all of the data to the device. This means that constructing
    /// stores from the builder involves no copying of data from the CPU to the GPU, only within the GPU.
    pub async fn instantiate_module(
        &mut self,
        engine: &mut Engine,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        module: &Module,
        imports: Vec<NamedExtern<T>>,
    ) -> anyhow::Result<ModuleInstanceReferences<T>> {
        // Validation
        let validated_imports = module.typecheck_imports(&imports)?;

        // Predict the function pointers that we *will* be creating, for ref evaluation
        let predicted_func_ptrs = module.predict_functions(&self.functions);

        // Globals
        let global_ptrs = module
            .try_initialize_globals(
                queue,
                &mut self.mutable_globals,
                &mut self.immutable_globals,
                validated_imports.globals().map(|p| p.clone()),
                &predicted_func_ptrs,
            )
            .await?;

        // Elements
        let element_ptrs = module
            .try_initialize_elements(
                queue,
                &mut self.elements,
                &mut self.mutable_globals,
                &mut self.immutable_globals,
                &global_ptrs,
                &predicted_func_ptrs,
            )
            .await?;

        // Tables
        let table_ptrs = module
            .try_initialize_tables(
                queue,
                &mut self.tables,
                validated_imports.tables(),
                &mut self.elements,
                &element_ptrs,
                &mut self.mutable_globals,
                &mut self.immutable_globals,
                &global_ptrs,
                &predicted_func_ptrs,
            )
            .await?;

        // Datas
        let data_ptrs = module.try_initialize_datas(queue, &mut self.datas).await?;

        // Memories
        let memory_ptrs = module
            .try_initialize_memories(
                queue,
                &mut self.memories,
                validated_imports.memories(),
                &mut self.datas,
                &data_ptrs,
                &mut self.mutable_globals,
                &mut self.immutable_globals,
                &global_ptrs,
                &predicted_func_ptrs,
            )
            .await?;

        // Functions
        let func_ptrs = module
            .try_initialize_functions(engine, &mut self.functions, validated_imports.functions())
            .await?;
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
        return Ok(ModuleInstanceReferences::new(
            func_ptrs,
            table_ptrs,
            memory_ptrs,
            global_ptrs,
            exports,
            start_fn,
        ));
    }

    pub fn register_function(&mut self, func: Func<T>) -> UntypedFuncPtr<T> {
        return self.functions.register(func);
    }

    /// Takes this builder and makes it immutable, allowing instances to be created from it
    pub async fn complete(
        self,
        queue: &AsyncQueue,
    ) -> Result<CompletedBuilder<T>, DelayedOutOfMemoryError<Self>> {
        let UnmappedStoreSetBuilder {
            tables,
            memories,
            mutable_globals,
            elements,
            datas,
            immutable_globals,
            functions,
        } = self.try_unmap(queue).await?;

        let assembled_module = AssembledModule::assemble(&functions);

        Ok(CompletedBuilder {
            tables,
            memories,
            mutable_globals,
            elements: Arc::new(elements),
            immutable_globals: Arc::new(immutable_globals),
            datas: Arc::new(datas),
            functions: Arc::new(functions),
            assembled_module: Arc::new(assembled_module),
        })
    }
}

pub struct CompletedBuilder<T> {
    tables: UnmappedTableInstanceSetBuilder,
    memories: UnmappedMemoryInstanceSetBuilder,
    mutable_globals: UnmappedMutableGlobalsInstanceBuilder,
    immutable_globals: Arc<UnmappedImmutableGlobalsInstance>,
    elements: Arc<UnmappedElementInstance>,
    datas: Arc<UnmappedDataInstance>,
    functions: Arc<FuncsInstance<T>>,

    /// We build the actual spir-v at builder completion, then copy it out to all store sets as they're
    /// instantiated. However all the information for the module can be rebuilt from the above data, so
    /// hoisting this is for optimisation reasons.
    assembled_module: Arc<AssembledModule>,
}

impl<T> CompletedBuilder<T> {
    /// Takes the instructions provided to this builder and produces a collection of stores which can
    /// be used to evaluate instructions. W take all of the initialisation that we did that can be shared and
    /// spin it into several instances. This shouldn't involve moving any data to the device, instead data
    /// that has already been provided to the device should be cloned and specialised as needed for a
    /// collection of instances.
    pub async fn build(
        &self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        values: impl IntoIterator<Item = T>,
    ) -> Result<DeviceStoreSet<T>, OutOfMemoryError> {
        let data: Vec<_> = values.into_iter().collect();

        let tables = self
            .tables
            .try_build(memory_system, queue, data.len())
            .await?;

        let memories = self
            .memories
            .try_build(memory_system, queue, data.len())
            .await?;

        let mutable_globals = self
            .mutable_globals
            .try_build(memory_system, queue, data.len())
            .await?;

        Ok(DeviceStoreSet {
            data,
            functions: self.functions.clone(),
            elements: self.elements.clone(),
            datas: self.datas.clone(),
            immutable_globals: self.immutable_globals.clone(),
            assembled_module: self.assembled_module.clone(),
            owned: UnmappedStoreSetData {
                tables,
                memories,
                mutable_globals,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::tests_lib::{gen_test_memory_string, get_backend};
    use crate::{block_test, imports, wasp, Config, Engine, MappedStoreSetBuilder};
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
        let (memory_system, queue) = get_backend().await;

        let (expected_data, data_str) = gen_test_memory_string(size, 84637322u32);

        let mut engine = Engine::new(Config::default());

        let mut stores_builder = MappedStoreSetBuilder::<()>::new(&memory_system);

        let wat = format!(
            r#"
            (module
                (data "{}")
            )
        "#,
            data_str
        );
        let wat = wat.into_bytes();
        let module = wasp::Module::new(&engine, &wat, "test_module".to_owned()).unwrap();

        let _instance = stores_builder
            .instantiate_module(&mut engine, &memory_system, &queue, &module, imports! {})
            .await
            .expect("could not instantiate all modules");

        let set = stores_builder.complete(&queue).await.unwrap();

        let buffers = Arc::try_unwrap(set.datas)
            .map_err(|_| {
                anyhow!("multiple references existed to buffer that should probably be owned")
            })
            .unwrap();

        assert_eq!(buffers.try_read_all(&queue).await.unwrap(), expected_data)
    }
}
