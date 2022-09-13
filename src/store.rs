use crate::atomic_counter::AtomicCounter;
use crate::func::TypedFuncPtr;
use crate::read_only::ReadOnly;
use crate::typed::{WasmTyVal, WasmTyVec};
use crate::{Backend, Engine, Extern, Func, Module};
use anyhow::{anyhow, Context};
use elsa::sync::{FrozenMap, FrozenVec};
use itertools::Itertools;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use wasmtime::{FuncType, ValType, WasmParams};

static STORE_COUNTER: AtomicCounter = AtomicCounter::new(); // Use as store hash & equality

pub enum Export<B, T>
where
    B: Backend,
{
    Func(FuncPtr<B, T>),
}

/// All of the state for one WASM state machine
/// Treated like an arena - everything allocated is allocated in here, and everyone else just keeps
/// 'references' in the form of usize indices into the FrozenVecs
pub struct Store<B, T>
where
    B: Backend,
{
    backend: Arc<B>,
    data: T,

    functions: FrozenVec<Arc<ReadOnly<Func<B, T>>>>,
    exports: FrozenMap<String, Box<Export<B, T>>>,

    id: usize,
}

impl<B, T> Store<B, T>
where
    B: Backend,
{
    pub fn data(&self) -> &T {
        return &self.data;
    }
}

/// A func in a store
pub struct FuncPtr<B, T>
where
    B: Backend,
{
    // Only make sense in the context of a specific store
    func_ptr: usize,
    store_id: usize,

    // Copied from Func
    ty: FuncType,

    _phantom_data: PhantomData<(B, T)>,
}

impl<B, T> FuncPtr<B, T>
where
    B: Backend,
{
    pub fn params(&self) -> impl ExactSizeIterator<Item = ValType> + '_ {
        return self.ty.params();
    }

    pub fn results(&self) -> impl ExactSizeIterator<Item = ValType> + '_ {
        return self.ty.results();
    }
}

impl<B, T> Clone for FuncPtr<B, T>
where
    B: Backend,
{
    fn clone(&self) -> Self {
        Self {
            func_ptr: self.func_ptr,
            store_id: self.store_id,
            ty: self.ty.clone(),
            _phantom_data: Default::default(),
        }
    }
}

impl<B, T> Hash for FuncPtr<B, T>
where
    B: Backend,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.store_id);
        state.write_usize(self.func_ptr);
    }
}

impl<B, T> PartialEq<Self> for FuncPtr<B, T>
where
    B: Backend,
{
    fn eq(&self, other: &Self) -> bool {
        self.store_id == other.store_id && self.func_ptr == other.func_ptr
    }
}

impl<B, T> Eq for FuncPtr<B, T> where B: Backend {}

impl<B, T> PartialOrd<Self> for FuncPtr<B, T>
where
    B: Backend,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<B, T> Ord for FuncPtr<B, T>
where
    B: Backend,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.store_id.cmp(&other.store_id) {
            std::cmp::Ordering::Equal => self.func_ptr.cmp(&other.func_ptr),
            v => v,
        }
    }
}

impl<B, T> Store<B, T>
where
    B: Backend,
{
    fn new(backend: Arc<B>, data: T) -> Self {
        Self {
            backend,
            data,
            functions: FrozenVec::new(),
            exports: FrozenMap::new(),
            id: STORE_COUNTER.next(),
        }
    }

    pub fn register_function(&self, func: Arc<ReadOnly<Func<B, T>>>) -> FuncPtr<B, T> {
        let ty = func.read().unwrap().ty();
        let func_ptr = self.functions.push_get_index(func);

        return FuncPtr {
            func_ptr,
            store_id: self.id,
            ty,
            _phantom_data: Default::default(),
        };
    }

    /// Create an exported function that doesn't track its types, useful for runtime imports.
    /// Prefer get_typed_func if possible, and see get_typed_func for detail
    /// about this function.
    pub fn get_func(&self, name: &str) -> anyhow::Result<FuncPtr<B, T>> {
        self.exports
            .get(name)
            .ok_or(anyhow!("no exported object with name {}", name))
            .and_then(|export| match export {
                Export::Func(f) => Ok(f.clone()),
                _ => Err(anyhow!("exported object named {} is not a function", name)),
            })
    }

    /// Create an exported function that tracks its types.
    /// Prefer calling once and reusing the returned exported function.
    pub fn get_typed_func<Params, Results>(
        &self,
        name: &str,
    ) -> anyhow::Result<TypedFuncPtr<B, T, Params, Results>>
    where
        Params: WasmTyVec,
        Results: WasmTyVec,
    {
        let untyped = self
            .get_func(name)
            .context(format!("failed to find function export `{}`", name))?;
        let typed = TypedFuncPtr::<B, T, Params, Results>::try_from(untyped).context(format!(
            "failed to convert function `{}` to given type",
            name
        ))?;

        return Ok(typed);
    }
}

impl<B, T> Hash for Store<B, T>
where
    B: Backend,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.id)
    }
}

impl<B, T> PartialEq<Self> for Store<B, T>
where
    B: Backend,
{
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<B, T> Eq for Store<B, T> where B: Backend {}

impl<B, T> PartialOrd<Self> for Store<B, T>
where
    B: Backend,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<B, T> Ord for Store<B, T>
where
    B: Backend,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

pub struct StoreSet<B, T>
where
    B: Backend,
{
    backend: Arc<B>,
    stores: BTreeMap<usize, Store<B, T>>, // stored by store::id
}

pub struct StoreSetFilterResult<B, T>
where
    B: Backend,
{
    pub true_stores: StoreSet<B, T>,
    pub false_stores: StoreSet<B, T>,
}

impl<'a, B: 'a, T: 'a> StoreSet<B, T>
where
    B: Backend,
{
    pub fn new(engine: &Engine<B>, range: impl Iterator<Item = T>) -> Self {
        Self {
            stores: range
                .map(|v| Store::new(engine.backend(), v))
                .map(|store| (store.id, store))
                .collect(),
            backend: engine.backend(),
        }
    }

    pub fn backend(&self) -> Arc<B> {
        self.backend.clone()
    }

    pub fn split<P>(self, mut predicate: P) -> StoreSetFilterResult<B, T>
    where
        P: FnMut(&T) -> bool,
    {
        let mut true_stores = BTreeMap::new();
        let mut false_stores = BTreeMap::new();

        for (id, store) in self.stores.into_iter() {
            assert_eq!(id, store.id);
            if predicate(&store.data) {
                true_stores.insert(id, store);
            } else {
                false_stores.insert(id, store);
            }
        }

        return StoreSetFilterResult {
            true_stores: Self {
                backend: self.backend.clone(),
                stores: true_stores,
            },
            false_stores: Self {
                backend: self.backend,
                stores: false_stores,
            },
        };
    }

    pub fn union(self, other: Self) -> Result<Self, (anyhow::Error, Self, Self)> {
        if !Arc::ptr_eq(&self.backend, &other.backend) {
            return Err((
                anyhow!("attempted to union two store sets with different backend instances"),
                self,
                other,
            ));
        }
        let mut stores = self.stores;
        stores.extend(other.stores);
        return Ok(Self {
            backend: self.backend,
            stores,
        });
    }

    pub async fn instantiate_module(
        &self,
        module: &Module<B>,
        imports: &[impl IntoIterator<Item = impl Into<Extern<B, T>>>],
    ) -> Vec<anyhow::Result<()>> {
        todo!()
    }

    pub fn register_function(&self, func: Func<B, T>) -> Vec<FuncPtr<B, T>> {
        let func = Arc::new(ReadOnly::new(func));
        return self
            .stores
            .iter()
            .map(|(_, store)| store.register_function(func.clone()))
            .collect_vec();
    }

    pub fn contains(&self, store: &Store<B, T>) -> bool {
        self.stores.contains_key(&store.id)
    }

    pub fn funcs_stores(
        &mut self,
        func_ptrs: impl IntoIterator<Item = &'a FuncPtr<B, T>>,
    ) -> Vec<&mut Store<B, T>> {
        let ordering: Vec<usize> = func_ptrs
            .into_iter()
            .map(|fp: &FuncPtr<B, T>| fp.store_id)
            .collect();

        let keys: HashSet<usize> = ordering.iter().map(usize::clone).collect();
        let mut refs: HashMap<usize, Option<&mut Store<B, T>>> = self
            .stores
            .iter_mut()
            .filter(|(store_id, store)| {
                assert_eq!(**store_id, store.id);
                keys.contains(store_id)
            })
            .map(|(id, store)| (id.clone(), Some(store)))
            .collect();

        return ordering
            .into_iter()
            .map(|id| {
                refs.get_mut(&id)
                    .expect("func pointer store was not present in store set")
                    .take()
                    .expect("two function pointers pointed into the same store")
            })
            .collect_vec();
    }

    pub fn get_funcs(&self, name: &str) -> Vec<anyhow::Result<FuncPtr<B, T>>> {
        self.stores
            .iter()
            .map(|(_, s)| s.get_func(name))
            .collect_vec()
    }

    pub fn get_typed_funcs<Params, Results>(
        &self,
        name: &str,
    ) -> Vec<anyhow::Result<TypedFuncPtr<B, T, Params, Results>>>
    where
        Params: WasmTyVec,
        Results: WasmTyVec,
    {
        self.stores
            .iter()
            .map(|(_, s)| s.get_typed_func(name))
            .collect_vec()
    }
}
