use crate::atomic_counter::AtomicCounter;
use crate::externs::NamedExtern;
use crate::func::TypedFuncPtr;
use crate::global_instance::{GlobalInstance, GlobalType};
use crate::instance::ModuleInstance;
use crate::memory::{DynamicMemoryBlock, Memory};
use crate::read_only::{AppendOnlyVec, ReadOnly};
use crate::store::ptrs::{FuncPtr, MemoryPtr, StorePtr};
use crate::typed::WasmTyVec;
use crate::{Backend, Engine, Extern, Func, Module, StoreSet};
use anyhow::{anyhow, Context};
use elsa::sync::{FrozenMap, FrozenVec};
use itertools::Itertools;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
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

    functions: Arc<AppendOnlyVec<Func<B, T>>>,
    tables: Vec<()>,
    memories: Vec<DynamicMemoryBlock<B>>,
    globals: GlobalInstance<B>,
    elements: Arc<AppendOnlyVec<()>>,
    datas: Arc<AppendOnlyVec<DynamicMemoryBlock<B>>>,

    id: usize,
}

impl<'a, B: 'a, T: 'a> StoreSetBuilder<B, T>
where
    B: Backend,
{
    pub fn new(engine: &Engine<B>) -> Self {
        let id = STORE_SET_COUNTER.next();
        Self {
            backend: engine.backend(),

            functions: Arc::new(AppendOnlyVec::new()),
            tables: Vec::new(),
            memories: Vec::new(),
            globals: GlobalInstance::new(engine.backend(), id),
            elements: Arc::new(AppendOnlyVec::new()),
            datas: Arc::new(AppendOnlyVec::new()),

            id,
        }
    }

    pub fn backend(&self) -> Arc<B> {
        self.backend.clone()
    }

    pub async fn instantiate_module(
        &mut self,
        module: &Module<B>,
        imports: impl IntoIterator<Item = NamedExtern<B, T>>,
    ) -> anyhow::Result<ModuleInstance<B, T>> {
        // Instantiation 1-4
        let validated_imports = module.typecheck_imports(imports)?;

        // Instantiation 5: Globals - shared instantiation which copies later
        let global_ptrs = module
            .initialize_globals(&mut self.globals, validated_imports.globals())
            .await;

        return Ok(ModuleInstance {});
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
