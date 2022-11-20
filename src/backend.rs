pub mod lazy;

use async_trait::async_trait;
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
    MapError(<B::DeviceMemoryBlock as crate::memory::DeviceMemoryBlock<B>>::MapError),
}

#[async_trait]
pub trait Backend: Sized + Debug + Sync {
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

    async fn try_create_and_map_empty(
        &self,
    ) -> Result<Self::MainMemoryBlock, AllocOrMapFailure<Self>> {
        self.try_create_device_memory_block(0, None)
            .map_err(AllocOrMapFailure::AllocError)?
            .map()
            .await
            .map_err(|(e, _)| AllocOrMapFailure::MapError(e))
    }
}
