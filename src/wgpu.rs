mod buffer_ring;
mod memory;

use crate::atomic_counter::AtomicCounter;
use crate::memory::{DeviceMemoryBlock, MainMemoryBlock};
use crate::wgpu::buffer_ring::BufferRing;
use crate::wgpu::memory::{WgpuMappedMemoryBlock, WgpuUnmappedMemoryBlock};
use crate::Backend;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use wgpu::{Device, Maintain, MaintainBase, MapMode, Queue};

struct WgpuFuture {
    device: Arc<Device>,
    done: AtomicBool,
}

impl WgpuFuture {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            done: AtomicBool::new(false),
        }
    }

    pub fn complete(&self) {
        self.done.store(true, Ordering::Release);
    }
}

impl Future for WgpuFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.done.load(Ordering::Acquire) {
            return Poll::Ready(());
        }

        self.device.poll(Maintain::Poll);

        return Poll::Pending;
    }
}

pub struct WgpuBackend {
    device: Arc<Device>,
    queue: Arc<Queue>,

    upload_buffers: Arc<BufferRing>,
    download_buffers: Arc<BufferRing>,

    block_counter: AtomicCounter,
}

impl WgpuBackend {
    pub fn new(device: Device, queue: Queue) -> Self {
        let device = Arc::new(device);
        let queue = Arc::new(queue);
        Self {
            upload_buffers: Arc::new(BufferRing::new(
                device.clone(),
                "Upload".to_owned(),
                MapMode::Write,
            )),
            download_buffers: Arc::new(BufferRing::new(
                device.clone(),
                "Download".to_owned(),
                MapMode::Read,
            )),
            queue,
            device,
            block_counter: AtomicCounter::new(),
        }
    }
}

impl Backend for WgpuBackend {
    type DeviceMemoryBlock = WgpuUnmappedMemoryBlock;
    type MainMemoryBlock = WgpuMappedMemoryBlock;

    fn create_device_memory_block(
        &mut self,
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
}
