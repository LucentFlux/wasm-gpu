use crate::memory::{DeviceMemoryBlock, MainMemoryBlock};

pub trait Backend: Sized {
    type DeviceMemoryBlock: crate::memory::DeviceMemoryBlock<Self>;
    type MainMemoryBlock: crate::memory::MainMemoryBlock<Self>;

    fn create_device_memory_block(
        &mut self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> dyn DeviceMemoryBlock<Self>;
}
