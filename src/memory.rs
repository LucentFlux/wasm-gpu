//! Data can be either on the CPU or parallel computing device. Due to the existence of host functions,
//! we need to be able to map between these locations at runtime, with minimal overhead. This file deals with
//! these types

pub mod interleaved;

use crate::Backend;
use std::ops::RangeBounds;
use std::ptr;

use crate::typed::ToRange;
use async_trait::async_trait;

#[async_trait]
pub trait MemoryBlock<B>
where
    B: Backend,
{
    fn backend(&self) -> &B;

    fn len(&self) -> usize;
}

#[async_trait]
pub trait MainMemoryBlock<B>: MemoryBlock<B>
where
    B: Backend,
{
    async fn as_slice<S: ToRange<usize> + Send>(&self, bounds: S) -> &[u8];
    async fn as_slice_mut<S: ToRange<usize> + Send>(&mut self, bounds: S) -> &mut [u8];
    async fn move_to_device_memory(self) -> B::DeviceMemoryBlock;

    /// Convenience method for writing blocks of data
    async fn write(&mut self, data: &[u8], offset: usize) {
        let start = offset;
        let end = start + data.len();
        let slice = self.as_slice_mut(start..end).await;
        slice.copy_from_slice(data);
    }

    /// Resizes by moving off of main memory, reallocating and copying
    async fn flush_resize(&mut self, new_len: usize) {
        // We want to perform mem::replace, but with a value that doesn't exist yet, and do do this
        // async. This functionality doesn't yet exist in rust
        unsafe {
            let tmp_mapped = ptr::read(self);

            // Must not panic before we get to `ptr::write`
            let mut tmp_unmapped = tmp_mapped.move_to_device_memory().await;
            tmp_unmapped.resize(new_len).await;
            let tmp_mapped = tmp_unmapped.move_to_main_memory().await;

            ptr::write(self, tmp_mapped);
        }
    }

    /// Convenience wrapper around `flush_resize` that adds more space
    async fn flush_extend(&mut self, extra: usize) {
        let len = self.len();
        self.flush_resize(len + extra).await
    }
}

#[async_trait]
pub trait DeviceMemoryBlock<B>: MemoryBlock<B> + Sized
where
    B: Backend,
{
    async fn move_to_main_memory(self) -> B::MainMemoryBlock;
    async fn copy_from(&mut self, other: &B::DeviceMemoryBlock);

    /// Resizes by reallocation and copying
    async fn resize(&mut self, new_len: usize) {
        let backend = self.get_backend();
        let mut new_buffer = backend.create_device_memory_block(new_len, None);
        new_buffer.copy_from(&self);
        std::mem::swap(&mut self, &mut new_buffer);
    }

    /// Convenience wrapper around `resize` that adds more space
    async fn extend(&mut self, extra: usize) {
        let len = self.len();
        self.resize(len + extra).await;
    }
}

pub fn limits_match<V: Ord>(n1: V, m1: Option<V>, n2: V, m2: Option<V>) -> bool {
    if n1 > n2 {
        return false;
    }
    return match (m1, m2) {
        (None, None) => true,
        (Some(m1), Some(m2)) => (m1 >= m2),
        (_, _) => false,
    };
}
