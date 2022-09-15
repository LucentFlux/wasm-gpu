mod buffer_ring;
mod memory;

pub use crate::wgpu::buffer_ring::BufferRingConfig;

use crate::atomic_counter::AtomicCounter;
use crate::wgpu::buffer_ring::BufferRing;
use crate::wgpu::memory::{WgpuMappedMemoryBlock, WgpuUnmappedMemoryBlock};
use crate::Backend;
use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use wgpu::{Device, Maintain, MapMode, Queue};

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
        // Check with scoped lock
        {
            let mut lock = self.state.lock().expect("wgpu future was poisoned on poll");

            if let Some(res) = lock.result.take() {
                return Poll::Ready(res);
            }

            lock.waker = Some(cx.waker().clone());
        }

        self.device.poll(Maintain::Poll);

        // Treat as green thread - we pass back but are happy to sit in a spin loop and poll
        cx.waker().wake_by_ref();

        return Poll::Pending;
    }
}

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
    device: Arc<Device>,
    queue: Arc<Queue>,

    upload_buffers: Arc<BufferRing>,
    download_buffers: Arc<BufferRing>,

    block_counter: AtomicCounter,
}

impl WgpuBackend {
    pub fn new(device: Device, queue: Queue, conf: WgpuBackendConfig) -> Self {
        let device = Arc::new(device);
        let queue = Arc::new(queue);
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
