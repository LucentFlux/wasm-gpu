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
use crate::wgpu::buffer_ring::{BufferRing, ConstMode};
use crate::wgpu::compute_utils::WgpuComputeUtils;
use crate::wgpu::memory::{WgpuMappedMemoryBlock, WgpuUnmappedMemoryBlock};
use crate::Backend;
use std::sync::Arc;
use wgpu::{Device, Queue};

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

pub struct WgpuBackend<const BUFFER_SIZE: usize> {
    device: AsyncDevice,
    queue: AsyncQueue,

    upload_buffers: Arc<BufferRing<BUFFER_SIZE, { ConstMode::Write }>>,
    download_buffers: Arc<BufferRing<BUFFER_SIZE, { ConstMode::Read }>>,

    block_counter: AtomicCounter,

    utils: WgpuComputeUtils,
}

impl<const BUFFER_SIZE: usize> WgpuBackend<BUFFER_SIZE> {
    pub fn new(device: Device, queue: Queue, conf: WgpuBackendConfig) -> Self {
        let device = AsyncDevice::new(device);
        let queue = AsyncQueue::new(device.clone(), queue);
        Self {
            upload_buffers: Arc::new(BufferRing::new(
                device.clone(),
                "Upload".to_owned(),
                conf.buffer_ring_config,
            )),
            download_buffers: Arc::new(BufferRing::new(
                device.clone(),
                "Download".to_owned(),
                conf.buffer_ring_config,
            )),
            utils: WgpuComputeUtils::new(device.clone()),
            queue,
            device,
            block_counter: AtomicCounter::new(),
        }
    }
}

impl<const BUFFER_SIZE: usize> Debug for WgpuBackend<BUFFER_SIZE> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "wgpu backend ({:?} with buffer size {})",
            self.device, BUFFER_SIZE
        )
    }
}

impl<const BUFFER_SIZE: usize> Backend for WgpuBackend<BUFFER_SIZE> {
    type DeviceMemoryBlock = WgpuUnmappedMemoryBlock<BUFFER_SIZE>;
    type MainMemoryBlock = WgpuMappedMemoryBlock<BUFFER_SIZE>;
    type Utils = WgpuComputeUtils;

    fn create_device_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> WgpuUnmappedMemoryBlock<BUFFER_SIZE> {
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
