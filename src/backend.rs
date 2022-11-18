pub mod lazy;

use crate::DeviceMemoryBlock;
use perfect_derive::perfect_derive;
use std::error::Error;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Error)]
#[perfect_derive(Debug)]
pub enum AllocOrMapFailure<B: Backend> {
    #[error("failed to allocate memory")]
    AllocError(B::BufferCreationError),
    #[error("failed to map memory on creation")]
    MapError(<B::DeviceMemoryBlock as DeviceMemoryBlock<B>>::MapError),
}

pub trait Backend: Sized + Debug {
    type BufferCreationError: Error + Send;

    type DeviceMemoryBlock: crate::memory::DeviceMemoryBlock<Self> + Send;
    type MainMemoryBlock: crate::memory::MainMemoryBlock<Self> + Send;
    type Utils: crate::compute_utils::Utils<Self>;

    fn try_create_device_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Result<Self::DeviceMemoryBlock, Self::BufferCreationError>;

    fn get_utils(&self) -> &Self::Utils;
}
