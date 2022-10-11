mod async_buffer;
mod async_device;
mod async_queue;
mod buffer_ring;
mod compute_utils;
mod memory;

pub use crate::wgpu::buffer_ring::BufferRingConfig;
use std::fmt::{Debug, Formatter};

use crate::atomic_counter::AtomicCounter;
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::async_queue::AsyncQueue;
use crate::wgpu::buffer_ring::BufferRing;
use crate::wgpu::compute_utils::WgpuComputeUtils;
use crate::wgpu::memory::{WgpuMappedMemoryBlock, WgpuUnmappedMemoryBlock};
use crate::Backend;
use std::sync::Arc;
use wgpu::{Device, MapMode, Queue};

#[derive(Copy, Clone)]
pub struct WgpuBackendConfig {
    pub buffer_ring_config: BufferRingConfig,
}

impl Default for WgpuBackendConfig {
    fn default() -> Self {
        Self {
            buffer_ring_config: Default::default(),
        }
    }
}

pub struct WgpuBackend {
    device: AsyncDevice,
    queue: AsyncQueue,

    upload_buffers: Arc<BufferRing>,
    download_buffers: Arc<BufferRing>,

    block_counter: AtomicCounter,

    utils: WgpuComputeUtils,
}

impl WgpuBackend {
    pub fn new(device: Device, queue: Queue, conf: WgpuBackendConfig) -> Self {
        let device = AsyncDevice::new(device);
        let queue = AsyncQueue::new(device.clone(), queue);
        Self {
            upload_buffers: Arc::new(BufferRing::new(
                device.clone(),
                "Upload".to_owned(),
                MapMode::Write,
                conf.buffer_ring_config,
            )),
            download_buffers: Arc::new(BufferRing::new(
                device.clone(),
                "Download".to_owned(),
                MapMode::Read,
                conf.buffer_ring_config,
            )),
            utils: WgpuComputeUtils::new(device.clone()),
            queue,
            device,
            block_counter: AtomicCounter::new(),
        }
    }
}

impl Debug for WgpuBackend {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "wgpu backend ({:?})", self.device)
    }
}

impl Backend for WgpuBackend {
    type DeviceMemoryBlock = WgpuUnmappedMemoryBlock;
    type MainMemoryBlock = WgpuMappedMemoryBlock;
    type Utils = WgpuComputeUtils;

    fn create_device_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> WgpuUnmappedMemoryBlock {
        WgpuUnmappedMemoryBlock::new(
            self.device.clone(),
            self.queue.clone(),
            self.upload_buffers.clone(),
            self.download_buffers.clone(),
            size,
            format!("Memory block {}", self.block_counter.next()),
            initial_data,
        )
    }

    fn get_utils(&self) -> &WgpuComputeUtils {
        &self.utils
    }
}
