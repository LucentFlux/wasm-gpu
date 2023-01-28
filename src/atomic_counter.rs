use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

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

#[derive(Debug)]
pub struct AtomicU32Counter(AtomicU32);

impl AtomicU32Counter {
    pub const fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    pub fn next(&self) -> u32 {
        self.0.fetch_add(1u32, Ordering::Relaxed)
    }
}
