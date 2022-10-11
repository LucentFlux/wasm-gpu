use crate::wgpu::async_buffer::AsyncBuffer;
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::async_queue::AsyncQueue;
use std::collections::VecDeque;
use std::future::Future;
use std::ops::RangeBounds;
use std::sync::{Arc, Mutex};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use wgpu::{BufferAddress, BufferDescriptor, BufferSlice, BufferUsages, MapMode};

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
    buffer: AsyncBuffer,
    map_mode: MapMode,
}

impl BufferRingBuffer {
    pub fn len(&self) -> usize {
        self.buffer_size
    }

    pub async fn map_slice<S: RangeBounds<BufferAddress>>(
        &self,
        bounds: S,
    ) -> anyhow::Result<BufferSlice> {
        return self.buffer.map_slice(bounds, self.map_mode).await;
    }

    /// Tell the GPU to move a chunk of data into this buffer
    async fn fill_from(
        &self,
        device: &AsyncDevice,
        queue: &AsyncQueue,
        src: &AsyncBuffer,
        offset: BufferAddress,
    ) {
        let mut copy_command_encoder = device.as_ref().create_command_encoder(&Default::default());
        copy_command_encoder.copy_buffer_to_buffer(
            src.as_ref(),
            offset,
            self.buffer.as_ref(),
            0,
            self.buffer_size as BufferAddress,
        );
        queue.submit(vec![copy_command_encoder.finish()]).await;
    }

    /// Tell the GPU to move a chunk of data into this buffer
    async fn write_to(
        &self,
        device: &AsyncDevice,
        queue: &AsyncQueue,
        dst: &AsyncBuffer,
        offset: BufferAddress,
    ) {
        let mut copy_command_encoder = device.as_ref().create_command_encoder(&Default::default());
        copy_command_encoder.copy_buffer_to_buffer(
            self.buffer.as_ref(),
            0,
            dst.as_ref(),
            offset,
            self.buffer_size as BufferAddress,
        );
        queue.submit(vec![copy_command_encoder.finish()]).await;
    }
}

pub struct BufferRing {
    config: BufferRingConfig,

    device: AsyncDevice,
    unused_buffers: Arc<Mutex<VecDeque<AsyncBuffer>>>,
    free_buffers: Arc<Semaphore>, // Tracks the above dequeue
    map_mode: MapMode,

    // Used for debugging
    label: String,
}

impl BufferRing {
    pub fn new(
        device: AsyncDevice,
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
            unused_buffers: Arc::new(Mutex::new(buffers)),
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
    async fn pop(&self) -> BufferRingBuffer {
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
            map_mode: self.map_mode,
        };
    }

    /// Buffer *must* have come from this ring. Executes in a tokio task
    fn push(&self, buffer: BufferRingBuffer) {
        let self_buffer_size = self.buffer_size();
        let self_map_mode = self.map_mode;
        let self_device = self.device.clone();
        let self_unused_buffers = self.unused_buffers.clone();
        tokio::task::spawn(async move || {
            let BufferRingBuffer {
                semaphore,
                buffer_size,
                buffer,
                map_mode,
            } = buffer;

            assert_eq!(buffer_size, self_buffer_size);
            assert_eq!(map_mode, self_map_mode);

            match self_map_mode {
                MapMode::Read => {
                    buffer.unmap();
                }
                MapMode::Write => {
                    self_device
                        .do_async(|callback| buffer.slice(..).map_async(MapMode::Write, callback))
                        .await
                        .expect("error mapping buffer");
                }
            };

            self_unused_buffers.lock().unwrap().push_back(buffer);

            drop(semaphore);
        });
    }

    /// Executes a closure with a slice of a GPU buffer.
    ///
    /// The slice generated has length this.buffer_size()
    ///
    /// # Panics
    /// Panics if this buffer ring's map mode is not Read
    pub async fn with_slice<Res, Fut: Future<Output = Res>, F: FnOnce(&[u8]) -> Fut>(
        &self,
        device: &AsyncDevice,
        queue: &AsyncQueue,
        src: &AsyncBuffer,
        offset: BufferAddress,
        f: F,
    ) -> Res {
        assert_eq!(self.map_mode, MapMode::Read);

        let download_buffer = self.pop().await;

        download_buffer.fill_from(device, queue, src, offset).await;

        let slice = download_buffer
            .map_slice(..self.buffer_size() as BufferAddress)
            .await
            .expect("failed to map download buffer");
        let view = slice.get_mapped_range();
        let view = view.as_ref();

        let res = f(view).await;

        self.push(download_buffer);

        return res;
    }

    /// Writes a slice to a GPU buffer.
    ///
    /// # Panics
    /// Panics if this buffer ring's map mode is not Write
    /// or if the src slice length is not exactly this.buffer_size()
    pub async fn write_slice(
        &self,
        device: &AsyncDevice,
        queue: &AsyncQueue,
        dst: &AsyncBuffer,
        offset: BufferAddress,
        src: &[u8],
    ) {
        assert_eq!(self.map_mode, MapMode::Write);
        assert_eq!(src.len(), self.buffer_size());

        let upload_buffer = self.pop().await;

        let slice = upload_buffer
            .map_slice(..self.buffer_size() as BufferAddress)
            .await
            .expect("failed to map upload buffer");
        slice.get_mapped_range_mut().copy_from_slice(src);

        upload_buffer.write_to(device, queue, dst, offset).await;

        self.push(upload_buffer);

        return res;
    }
}
