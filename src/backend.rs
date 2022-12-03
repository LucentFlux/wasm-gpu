pub mod lazy;

use async_trait::async_trait;
use std::error::Error;
use std::fmt::Debug;

#[async_trait]
pub trait Backend: Sized + Debug + Sync {
    type DeviceMemoryBlock: crate::memory::DeviceMemoryBlock<Self> + Send;
    type MainMemoryBlock: crate::memory::MainMemoryBlock<Self> + Send;
    type Utils: crate::compute_utils::Utils<Self>;

    fn create_device_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Self::DeviceMemoryBlock;

    fn get_utils(&self) -> &Self::Utils;

    async fn create_and_map_empty(&self) -> Self::MainMemoryBlock {
        self.create_device_memory_block(0, None).map().await
    }
}
