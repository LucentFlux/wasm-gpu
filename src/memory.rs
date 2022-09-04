//! Data can be either on the CPU or parallel computing device. Due to the existence of host functions,
//! we need to be able to map between these locations at runtime, with minimal overhead. This file deals with
//! these types

use crate::Backend;
use std::ops::RangeBounds;

use async_trait::async_trait;

#[async_trait]
pub trait MemoryBlock<B>
where
    B: Backend,
{
    async fn len(&self) -> usize;
}

#[async_trait]
pub trait MainMemoryBlock<B>: MemoryBlock<B>
where
    B: Backend,
{
    async fn as_slice<S: RangeBounds<usize>>(&mut self, bounds: S) -> &mut [u8];
    async fn move_to_device_memory(self) -> B::DeviceMemoryBlock;
}

#[async_trait]
pub trait DeviceMemoryBlock<B>: MemoryBlock<B>
where
    B: Backend,
{
    async fn move_to_main_memory(self) -> B::MainMemoryBlock;
}
