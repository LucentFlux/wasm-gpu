use stable_deref_trait::StableDeref;
use std::ops::Deref;
use std::sync::{Arc, LockResult, RwLock, RwLockReadGuard};

/// A read-only wrapper around a RwLock<T>.
pub struct ReadOnly<T> {
    inner: RwLock<T>,
}

impl<T> ReadOnly<T> {
    pub fn new(t: T) -> Self {
        Self {
            inner: RwLock::new(t),
        }
    }

    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        self.inner.read()
    }
}
