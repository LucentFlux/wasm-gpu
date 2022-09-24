use crate::store::store::Store;
use crate::Backend;

pub mod builder;
pub mod ptrs;
pub mod store;

pub struct StoreSet<B, T>
where
    B: Backend,
{
    stores: Vec<Store<B, T>>,
}

impl<B, T> StoreSet<B, T>
where
    B: Backend,
{
    /// Use StoreSetBuilder
    pub(crate) fn new(stores: Vec<Store<B, T>>) -> Self {
        Self { stores }
    }
}
