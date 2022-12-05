//! Data can be either on the CPU or parallel computing device. Due to the existence of host functions,
//! we need to be able to map between these locations at runtime, with minimal overhead. This file deals with
//! these types

pub mod interleaved;

use crate::Backend;
use std::fmt::Debug;

use crate::typed::ToRange;
use async_trait::async_trait;

#[async_trait]
pub trait MemoryBlock<B>: Debug
where
    B: Backend,
{
    fn backend(&self) -> &B;

    fn len(&self) -> usize;
}

#[async_trait]
pub trait MainMemoryBlock<B>: MemoryBlock<B> + Sized + Send
where
    B: Backend<MainMemoryBlock = Self>,
{
    async fn as_slice<S: ToRange<usize> + Send>(&self, bounds: S) -> &[u8];
    async fn as_slice_mut<S: ToRange<usize> + Send>(&mut self, bounds: S) -> &mut [u8];
    async fn unmap(self) -> B::DeviceMemoryBlock;

    /// Convenience method for writing blocks of data
    ///
    /// On error, no data was written.
    async fn write(&mut self, data: &[u8], offset: usize) {
        let start = offset;
        let end = start + data.len();
        let slice = self.as_slice_mut(start..end).await;
        slice.copy_from_slice(data);
    }

    /// Resizes by proxy, actual resize may only occur on unmap
    async fn resize(&mut self, new_len: usize);

    /// Convenience wrapper around `Self::resize` that adds more space
    async fn extend(&mut self, extra: usize) {
        let len = self.len();
        self.resize(len + extra).await
    }
}

#[async_trait]
pub trait DeviceMemoryBlock<B>: MemoryBlock<B> + Sized
where
    B: Backend<DeviceMemoryBlock = Self>,
{
    async fn map(self) -> B::MainMemoryBlock;
    async fn copy_from(&mut self, other: &B::DeviceMemoryBlock);

    /// Resizes by (at worst) reallocation and copying
    async fn resize(&mut self, new_len: usize) {
        let backend = self.backend();
        let mut new_buffer = backend.create_device_memory_block(new_len, None);
        let fut = new_buffer.copy_from(&self);
        fut.await;

        *self = new_buffer;
    }

    /// Convenience wrapper around `resize` that adds more space
    async fn extend(&mut self, extra: usize) {
        let len = self.len();
        self.resize(len + extra).await
    }
}

pub fn limits_match<V: Ord>(n1: V, m1: Option<V>, n2: V, m2: Option<V>) -> bool {
    if n1 > n2 {
        return false;
    }
    return match (m1, m2) {
        (None, None) => true,
        (Some(m1), Some(m2)) => m1 >= m2,
        (_, _) => false,
    };
}
