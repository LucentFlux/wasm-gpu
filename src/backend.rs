pub trait Backend: Sized {
    type DeviceMemoryBlock: crate::memory::DeviceMemoryBlock<Self>;
    type MainMemoryBlock: crate::memory::MainMemoryBlock<Self>;

    fn create_device_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Self::DeviceMemoryBlock;
}
