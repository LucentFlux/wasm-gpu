use crate::wgpu::async_buffer::AsyncBuffer;
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::async_queue::AsyncQueue;
use async_channel::{Receiver, Sender};
use std::ops::RangeBounds;
use wgpu::{BufferAddress, BufferDescriptor, BufferSlice, BufferUsages, MapMode};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ConstMode {
    Read,
    Write,
}

impl Into<MapMode> for ConstMode {
    fn into(self) -> MapMode {
        match self {
            ConstMode::Read => MapMode::Read,
            ConstMode::Write => MapMode::Write,
        }
    }
}

#[derive(Copy, Clone)]
pub struct BufferRingConfig {
    /// A ring will allocate this amount of memory for moving data
    pub total_mem: usize,
}

impl Default for BufferRingConfig {
    fn default() -> Self {
        Self {
            total_mem: 16 * 1024 * 1024, // 16MB
        }
    }
}

#[must_use]
pub struct BufferRingBuffer<const SIZE: usize, const MODE: ConstMode> {
    buffer: AsyncBuffer,
}

impl<const SIZE: usize, const MODE: ConstMode> BufferRingBuffer<SIZE, MODE> {
    pub fn len(&self) -> usize {
        SIZE
    }

    pub async fn map_slice<S: RangeBounds<BufferAddress>>(
        &self,
        bounds: S,
    ) -> anyhow::Result<BufferSlice> {
        return self.buffer.map_slice(bounds, MODE.into()).await;
    }
}

impl<const SIZE: usize> BufferRingBuffer<SIZE, { ConstMode::Write }> {
    /// Tell the GPU to move a chunk of data from this buffer into another buffer
    async fn copy_to(
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
            SIZE as BufferAddress,
        );
        queue.submit(vec![copy_command_encoder.finish()]).await;
    }
}

impl<const SIZE: usize> BufferRingBuffer<SIZE, { ConstMode::Read }> {
    /// Tell the GPU to move a chunk of data into this buffer
    async fn copy_from(
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
            SIZE as BufferAddress,
        );
        queue.submit(vec![copy_command_encoder.finish()]).await;
    }
}

pub struct BufferRing<const SIZE: usize, const MODE: ConstMode> {
    config: BufferRingConfig,

    device: AsyncDevice,
    unused_buffers: Receiver<AsyncBuffer>,
    buffer_return: Sender<AsyncBuffer>,

    // Used for debugging
    label: String,
}

impl<const SIZE: usize, const MODE: ConstMode> BufferRing<SIZE, MODE> {
    pub fn new(device: AsyncDevice, label: String, config: BufferRingConfig) -> Self {
        let buffer_count = config.total_mem / SIZE;
        let (buffer_return, unused_buffers) = async_channel::bounded(buffer_count);
        for buffer_id in 0..buffer_count {
            let new_buffer = device.create_buffer(&BufferDescriptor {
                label: Some(format!("Staging buffer [{} #{}]", label, buffer_id).as_str()),
                size: SIZE as BufferAddress,
                usage: Self::usages(MODE.into()),
                mapped_at_creation: Self::mapped_at_creation(MODE.into()),
            });

            // Future should immediately resolve since we reserved space
            let fut = buffer_return.send(new_buffer);
            futures::executor::block_on(fut).unwrap()
        }

        Self {
            config,
            device,
            unused_buffers,
            buffer_return,
            label,
        }
    }

    const fn mapped_at_creation(map_mode: MapMode) -> bool {
        match map_mode {
            MapMode::Read => false,
            MapMode::Write => true,
        }
    }

    const fn usages(map_mode: MapMode) -> BufferUsages {
        match map_mode {
            MapMode::Read => BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            MapMode::Write => BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC,
        }
    }

    pub const fn buffer_size(&self) -> usize {
        SIZE
    }

    /// Gets a new buffer of size STAGING_BUFFER_SIZE. If map_mode is MapMode::Write, then the whole
    /// buffer is already mapped to CPU memory
    async fn pop(&self) -> BufferRingBuffer<SIZE, MODE> {
        let buffer = self.unused_buffers.recv().await.unwrap();

        return BufferRingBuffer { buffer };
    }

    /// Buffer *must* have come from this ring. Executes in a tokio task
    fn push(&self, buffer: BufferRingBuffer<SIZE, MODE>) {
        let ret = self.buffer_return.clone();
        tokio::task::spawn(async {
            let BufferRingBuffer { buffer } = buffer;

            match MODE {
                ConstMode::Read => {
                    buffer.as_ref().unmap();
                }
                ConstMode::Write => {
                    buffer
                        .map_slice(.., MapMode::Write)
                        .await
                        .expect("error mapping buffer");
                }
            };

            ret.send(buffer).await.unwrap();
        });
    }
}

impl<const BUFFER_SIZE: usize> BufferRing<BUFFER_SIZE, { ConstMode::Read }> {
    /// Executes a closure with a slice of a GPU buffer.
    ///
    /// The slice generated has length BUFFER_SIZE
    pub async fn with_slice<'a, Res, F: FnOnce(&[u8]) -> Res>(
        &'a self,
        device: &AsyncDevice,
        queue: &AsyncQueue,
        src: &AsyncBuffer,
        offset: BufferAddress,
        f: F,
    ) -> Res {
        let download_buffer = self.pop().await;

        download_buffer.copy_from(device, queue, src, offset).await;

        let res = {
            let slice = download_buffer
                .map_slice(..self.buffer_size() as BufferAddress)
                .await
                .expect("failed to map download buffer");
            let view = slice.get_mapped_range();
            let view = view.as_ref();

            f(view)
        };

        self.push(download_buffer);

        return res;
    }
}

impl<const BUFFER_SIZE: usize> BufferRing<BUFFER_SIZE, { ConstMode::Write }> {
    /// Writes a slice to a GPU buffer.
    pub async fn write_slice(
        &self,
        device: &AsyncDevice,
        queue: &AsyncQueue,
        dst: &AsyncBuffer,
        offset: BufferAddress,
        src: &[u8; BUFFER_SIZE],
    ) {
        let upload_buffer = self.pop().await;

        let slice = upload_buffer
            .map_slice(..self.buffer_size() as BufferAddress)
            .await
            .expect("failed to map upload buffer");
        slice.get_mapped_range_mut().copy_from_slice(src);

        upload_buffer.copy_to(device, queue, dst, offset).await;

        self.push(upload_buffer);
    }
}
