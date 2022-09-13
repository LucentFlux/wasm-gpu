mod buffer_ring;
mod memory;

use crate::atomic_counter::AtomicCounter;
use crate::memory::{DeviceMemoryBlock, MainMemoryBlock};
use crate::wgpu::buffer_ring::BufferRing;
use crate::wgpu::memory::{WgpuMappedMemoryBlock, WgpuUnmappedMemoryBlock};
use crate::Backend;
use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use wgpu::{Device, Maintain, MaintainBase, MapMode, Queue};

struct WgpuFutureSharedState<T> {
    result: Option<T>,
    waker: Option<Waker>,
}

struct WgpuFuture<T> {
    device: Arc<Device>,
    state: Arc<Mutex<WgpuFutureSharedState<T>>>,
}

impl<T: Send + 'static> WgpuFuture<T> {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            state: Arc::new(Mutex::new(WgpuFutureSharedState {
                result: None,
                waker: None,
            })),
        }
    }

    pub fn callback(&self) -> Box<dyn FnOnce(T) -> () + Send + 'static> {
        let shared_state = self.state.clone();
        return Box::new(move |res: T| {
            let mut lock = shared_state
                .lock()
                .expect("wgpu future was poisoned on complete");
            let shared_state = lock.deref_mut();
            shared_state.result = Some(res);

            if let Some(waker) = shared_state.waker.take() {
                waker.wake()
            }
        });
    }
}

impl<T> Future for WgpuFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut lock = self.state.lock().expect("wgpu future was poisoned on poll");

        if let Some(res) = lock.result.take() {
            return Poll::Ready(res);
        }

        lock.waker = Some(cx.waker().clone());

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
