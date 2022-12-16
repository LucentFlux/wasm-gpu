use crate::externs::NamedExtern;
use crate::instance::data::{MappedDataInstance, UnmappedDataInstance};
use crate::instance::element::{MappedElementInstance, UnmappedElementInstance};
use crate::instance::func::{FuncsInstance, UntypedFuncPtr};
use crate::instance::global::builder::{
    AbstractGlobalPtr, MappedMutableGlobalInstanceBuilder, UnmappedMutableGlobalInstanceBuilder,
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
use crate::store_set::DeviceStoreSetData;
use crate::{DeviceStoreSet, ExternRef, Func, FuncRef, Ieee32, Ieee64, Module, Val};
use lf_hal::backend::Backend;
use std::future::join;
use std::sync::Arc;
use wasmparser::{Global, Operator, ValType};

/// Acts like a traditional OOP factory where we initialise modules into this before
/// creating single Stores after all initialization is done, to amortize the instantiation cost
pub struct StoreSetBuilder<B, T>
where
    B: Backend,
{
    backend: Arc<B>,

    tables: MappedTableInstanceSetBuilder<B>,
    memories: MappedMemoryInstanceSetBuilder<B>,
    mutable_globals: MappedMutableGlobalInstanceBuilder<B>,
    // Immutable so don't need to be abstr
    elements: MappedElementInstance<B>,
    datas: MappedDataInstance<B>,
    functions: FuncsInstance<B, T>,
    immutable_globals: MappedImmutableGlobalsInstance<B>,
}

impl<B, T> StoreSetBuilder<B, T>
where
    B: Backend,
{
    pub async fn new(backend: Arc<B>) -> Self {
        let mutable_globals_fut = MappedMutableGlobalInstanceBuilder::new(backend.as_ref());
        let elements_fut = MappedElementInstance::new(backend.as_ref());
        let datas_fut = MappedDataInstance::new(backend.as_ref());

        let (mutable_globals, elements, datas) =
            join!(mutable_globals_fut, elements_fut, datas_fut).await;

        Self {
            functions: FuncsInstance::new(),
            tables: MappedTableInstanceSetBuilder::new(backend.clone()),
            memories: MappedMemoryInstanceSetBuilder::new(backend.clone()),
            immutable_globals: MappedImmutableGlobalsInstance::new(backend.clone()),
            mutable_globals,
            elements,
            datas,
            backend,
        }
    }

    /// Used during instantiation to evaluate an expression in a single pass
    pub async fn interpret_constexpr<'data, T>(
        &mut self,
        constr_expr: &Vec<Operator<'data>>,
        module: &ModuleInstanceReferences<B, T>,
    ) -> Val {
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
                    let function_ptr = module
                        .get_func_at(function_index)
                        .expect("function index out of range of module functions");
                    stack.push(Val::FuncRef(function_ptr.to_func_ref()))
                }
                Operator::GlobalGet { global_index } => {
                    let global_index = usize::try_from(*global_index).unwrap();
                    let global_ptr = module
                        .get_global_at(global_index)
                        .expect("global index out of range of module globals");
                    let global_val = match global_ptr {
                        AbstractGlobalPtr::Immutable(imm_ptr) => {
                            self.immutable_globals.get(imm_ptr).await
                        }
                        AbstractGlobalPtr::Mutable(mut_ptr) => {
                            self.mutable_globals.get(mut_ptr).await
                        }
                    };

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

        return res;
    }

    pub(crate) async fn snapshot(src: &DeviceStoreSet<B, T>, store_index: usize) -> Self {
        // We're doing this so you can execute a bit then load more modules, then execute some more.
        // See wizer for the idea origin.
        todo!()
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
    ) -> anyhow::Result<ModuleInstanceReferences<B, T>> {
        // Validation
        let validated_imports = module.typecheck_imports(&imports)?;

        // Predict the function pointers that we *will* be creating, for ref evaluation
        let predicted_func_ptrs = module.predict_functions(&self.functions);

        // Globals
        let global_ptrs = module
            .initialize_globals(
                &mut self.mutable_globals,
                validated_imports.globals().map(|p| p.clone()),
                &predicted_func_ptrs,
            )
            .await;

        // Elements
        let element_ptrs = module
            .initialize_elements(
                &mut self.elements,
                &mut self.mutable_globals,
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
                &mut self.mutable_globals,
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
                &mut self.mutable_globals,
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
        return Ok(ModuleInstanceReferences::new(
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

    pub async fn add_global<T>(
        &mut self,
        global: Global<'_>,
        module: &ModuleInstanceReferences<B, T>,
    ) -> AbstractGlobalPtr<B, T> {
        // Initialise
        let val = self.interpret_constexpr(&global.initializer, module).await;
        assert_eq!(
            val.get_type(),
            global.ty.content_type,
            "global evaluation had differing type to definition"
        );

        let ptr = if global.ty.mutable {
            AbstractGlobalPtr::Mutable(self.mutable_globals.push(val).await)
        } else {
            AbstractGlobalPtr::Immutable(self.immutable_globals.push(val).await)
        };

        return ptr;
    }

    /// Takes this builder and makes it immutable, allowing instances to be created from it
    pub async fn complete(self) -> CompletedBuilder<B, T> {
        let Self {
            backend,
            tables,
            memories,
            mutable_globals,
            immutable_globals,
            elements,
            datas,
            functions,
        } = self;

        let mutable_globals = mutable_globals.unmap().await;
        let immutable_globals = immutable_globals.unmap().await;
        let elements = elements.unmap().await;
        let datas = datas.unmap().await;
        let tables = tables.unmap().await;
        let memories = memories.unmap().await;
        CompletedBuilder {
            backend,
            tables,
            memories,
            mutable_globals,
            elements: Arc::new(elements),
            immutable_globals: Arc::new(immutable_globals),
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
    mutable_globals: UnmappedMutableGlobalInstanceBuilder<B>,
    immutable_globals: Arc<UnmappedImmutableGlobalsInstance<B>>,
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

        let mutable_globals = self.mutable_globals.build(data.len()).await;

        DeviceStoreSet {
            backend: self.backend.clone(),
            data,
            functions: self.functions.clone(),
            elements: self.elements.clone(),
            datas: self.datas.clone(),
            immutable_globals: self.immutable_globals.clone(),
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

        let mut stores_builder = StoreSetBuilder::<_, ()>::new(engine.backend()).await;

        let wat = format!(
            r#"
            (module
                (data "{}")
            )
        "#,
            data_str
        );
        let wat = wat.into_bytes();
        let module = wasp::Module::new(&engine, &wat).unwrap();

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
