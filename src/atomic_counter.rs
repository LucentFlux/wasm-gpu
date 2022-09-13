use std::sync::atomic::{AtomicUsize, Ordering};

pub struct AtomicCounter(AtomicUsize);

impl AtomicCounter {
    pub const fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    pub fn next(&self) -> usize {
        self.0.fetch_add(1usize, Ordering::Relaxed)
    }
}
