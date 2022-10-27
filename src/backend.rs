pub mod lazy;

use std::fmt::Debug;

pub trait Backend: Sized + Debug {
    type DeviceMemoryBlock: crate::memory::DeviceMemoryBlock<Self> + Send;
    type MainMemoryBlock: crate::memory::MainMemoryBlock<Self> + Send;
    type Utils: crate::compute_utils::Utils<Self>;

    fn create_device_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Self::DeviceMemoryBlock;

    fn get_utils(&self) -> &Self::Utils;
}
