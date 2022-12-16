use elsa::sync::FrozenVec;
use std::sync::{LockResult, RwLock, RwLockReadGuard};

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

/// A lockless list that doesn't allow mutable references of elements,
/// and doesn't require a mutable reference to append.
pub struct AppendOnlyVec<T: Clone> {
    inner: Vec<T>,
}

impl<T: Clone> AppendOnlyVec<T> {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn push_get_index(&self, val: T) -> usize {
        return self.inner.push(val);
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        return self.inner.get(i).map(|v| v.inner.read().unwrap());
    }
}

impl<T> FromIterator<T> for AppendOnlyVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let vec = FrozenVec::new();

        for i in iter.into_iter() {
            vec.push(i);
        }

        Self { inner: vec }
    }
}
