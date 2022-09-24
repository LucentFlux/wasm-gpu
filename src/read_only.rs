use elsa::sync::FrozenVec;
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

pub struct AppendOnlyVec<T> {
    inner: FrozenVec<Box<ReadOnly<T>>>,
}

impl<T> AppendOnlyVec<T> {
    pub fn new() -> Self {
        Self {
            inner: FrozenVec::new(),
        }
    }

    pub fn push_get_index(&self, val: T) -> usize {
        return self.inner.push_get_index(Box::new(ReadOnly::new(val)));
    }

    pub fn get(&self, i: usize) -> Option<RwLockReadGuard<T>> {
        return self.inner.get(i).map(|v| v.inner.read().unwrap());
    }
}
