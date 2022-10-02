use crate::store::ptrs::AbstractPtr;
pub use crate::store::store::Store;
use crate::Backend;
use std::sync::Arc;

pub mod builder;
pub mod ptrs;
pub mod store;

pub struct StoreSet<B, T>
where
    B: Backend,
{
    backend: Arc<B>,
    stores: Vec<Store<B, T>>,
}

impl<B, T> StoreSet<B, T>
where
    B: Backend,
{
    /// Use StoreSetBuilder
    pub(crate) fn new(backend: Arc<B>, stores: Vec<Store<B, T>>) -> Self {
        let same_backends = stores
            .iter()
            .map(|store| Arc::ptr_eq(&backend, &store.backend()))
            .all();
        assert!(same_backends);

        Self { backend, stores }
    }

    pub fn concrete<P: AbstractPtr>(&self, ptr: P) -> impl Iterator<Item = P::Concrete> {
        self.stores
            .iter()
            .map(|s| ptr.concrete(s.get_concrete_id()))
    }

    pub(crate) fn backend(&self) -> Arc<B> {
        self.backend.clone()
    }

    pub fn datas(&self) -> impl Iterator<Item = &T> {
        self.stores.iter().map(|s| s.data())
    }
}
