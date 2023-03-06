use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct AtomicUsizeCounter(AtomicUsize);

impl AtomicUsizeCounter {
    pub const fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    pub fn next(&self) -> usize {
        self.0.fetch_add(1usize, Ordering::Relaxed)
    }
}
