use crate::atomic_counter::AtomicCounter;
use crate::wgpu::async_buffer::AsyncBuffer;
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::WgpuFuture;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use wgpu::{BufferAddress, BufferDescriptor, BufferUsages, Maintain, MapMode};

#[derive(Copy, Clone)]
pub struct BufferRingConfig {
    /// A ring will allocate this amount of memory for moving data
    pub total_mem: usize,
    pub buffer_size: usize,
}

impl Default for BufferRingConfig {
    fn default() -> Self {
        Self {
            total_mem: 16 * 1024 * 1024, // 16MB
            buffer_size: 1024 * 1024,    // 1MB
        }
    }
}

#[must_use]
pub struct BufferRingBuffer {
    semaphore: OwnedSemaphorePermit,
    buffer_size: usize,
    pub(crate) buffer: AsyncBuffer,
}

impl BufferRingBuffer {
    pub fn len(&self) -> usize {
        self.buffer_size
    }
}

pub struct BufferRing {
    config: BufferRingConfig,

    device: Arc<AsyncDevice>,
    unused_buffers: Mutex<VecDeque<AsyncBuffer>>,
    free_buffers: Arc<Semaphore>, // Tracks the above dequeue
    map_mode: MapMode,

    // Used for debugging
    label: String,
}

impl BufferRing {
    pub fn new(
        device: Arc<AsyncDevice>,
        label: String,
        map_mode: MapMode,
        config: BufferRingConfig,
    ) -> Self {
        let buffer_count = config.total_mem / config.buffer_size;
        let mut buffers = VecDeque::new();
        for buffer_id in 0..buffer_count {
            let new_buffer = device.create_buffer(&BufferDescriptor {
                label: Some(format!("Staging buffer [{} #{}]", label, buffer_id).as_str()),
                size: config.buffer_size as BufferAddress,
                usage: Self::usages(map_mode),
                mapped_at_creation: Self::mapped_at_creation(map_mode),
            });
            buffers.push_back(new_buffer);
        }

        Self {
            free_buffers: Arc::new(Semaphore::new(buffer_count)),
            unused_buffers: Mutex::new(buffers),
            config,
            device,
            map_mode,
            label,
        }
    }

    fn mapped_at_creation(map_mode: MapMode) -> bool {
        match map_mode {
            MapMode::Read => false,
            MapMode::Write => true,
        }
    }

    fn usages(map_mode: MapMode) -> BufferUsages {
        match map_mode {
            MapMode::Read => BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            MapMode::Write => BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC,
        }
    }

    pub fn buffer_size(&self) -> usize {
        return self.config.buffer_size;
    }

    /// Gets a new buffer of size STAGING_BUFFER_SIZE. If map_mode is MapMode::Write, then the whole
    /// buffer is already mapped to CPU memory
    pub async fn pop(&self) -> BufferRingBuffer {
        let semaphore = self.free_buffers.clone().acquire_owned().await.unwrap();
        let buffer = self
            .unused_buffers
            .lock()
            .unwrap()
            .pop_front()
            .expect("semaphore count != buffer count");

        return BufferRingBuffer {
            semaphore,
            buffer_size: self.config.buffer_size,
            buffer,
        };
    }

    /// Buffer *must* have come from this ring
    pub async fn push(&self, buffer: BufferRingBuffer) {
        let BufferRingBuffer {
            semaphore, buffer, ..
        } = buffer;

        match self.map_mode {
            MapMode::Read => {
                buffer.unmap();
                future.callback()(Ok(()));
            }
            MapMode::Write => {
                self.device
                    .do_async(|callback| buffer.slice(..).map_async(MapMode::Write, callback))
                    .await
                    .expect("error mapping buffer");
            }
        };

        self.unused_buffers.lock().unwrap().push_back(buffer);
        drop(semaphore);
    }
}
