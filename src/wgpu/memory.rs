use crate::memory::{DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};
use crate::wgpu::buffer_ring::{BufferRing, STAGING_BUFFER_SIZE};
use crate::wgpu::WgpuFuture;
use crate::WgpuBackend;
use async_trait::async_trait;
use futures::future::join_all;
use pod::Pod;
use std::cmp::{max, min};
use std::collections::Bound;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::task::Poll::Pending;
use wast::kw::declare;
use wgpu::{BufferAddress, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, MapMode};

struct WgpuBufferMemoryBlock {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub upload_buffers: Arc<BufferRing>,
    pub download_buffers: Arc<BufferRing>,
    pub buffer: wgpu::Buffer, // Stored on the GPU
    pub len: usize,
}

#[async_trait]
impl MemoryBlock<WgpuBackend> for WgpuBufferMemoryBlock {
    async fn len(&self) -> usize {
        self.len
    }
}

/// Lazily downloaded chunks of data
struct LazyChunk {
    loaded: bool,
    data_len: usize,
    data_offset: usize,
}

impl LazyChunk {
    async fn download(&mut self, buffer: &mut [u8], block: &WgpuBufferMemoryBlock) {
        if self.loaded {
            return;
        }

        // Tell the GPU to move a chunk of data into the CPU accessible buffer
        let download_buffer = block.download_buffers.pop();
        let mut copy_command_encoder = block.device.create_command_encoder(&Default::default());
        copy_command_encoder.copy_buffer_to_buffer(
            &block.buffer,
            self.data_offset as BufferAddress,
            &download_buffer,
            0,
            self.data_len as BufferAddress,
        );
        block.queue.submit(vec![copy_command_encoder.finish()]);

        // Tell the GPU to, after the move, map the memory to the CPU
        let slice = download_buffer.slice(..length);
        let future = WgpuFuture::new(self.data.device.clone());
        slice.map_async(MapMode::Read, future.complete);

        // Wait for the above to complete
        future.await;

        // Copy the data that the GPU gave us into our buffer in ram
        let view = slice.get_mapped_range();
        buffer.copy_from_slice(view.as_ref());

        // Return our CPU accessible buffer
        block.download_buffers.push(download_buffer);

        self.loaded = true;
    }

    async fn upload(&mut self, buffer: &[u8], block: &WgpuBufferMemoryBlock) {
        if !self.loaded {
            return;
        }

        // Copy data into GPU accessible buffer
        let upload_buffer = block.upload_buffers.pop();
        let slice = upload_buffer.slice(..self.data_len);
        slice.get_mapped_range_mut().copy_from_slice(buffer);

        // Tell GPU to copy into non-CPU accessible buffer
        let mut copy_command_encoder = block.device.create_command_encoder(&Default::default());
        copy_command_encoder.copy_buffer_to_buffer(
            &upload_buffer,
            0,
            &block.buffer,
            self.data_offset as BufferAddress,
            self.data_len as BufferAddress,
        );
        block.queue.submit(vec![copy_command_encoder.finish()]);

        // Return our CPU accessible buffer
        block.upload_buffers.push(upload_buffer);

        self.loaded = false;
    }
}

struct LazyBuffer {
    data: Vec<u8>,
    chunks: Vec<LazyChunk>,
}

impl LazyBuffer {
    pub fn new(len: usize) -> Self {
        let mut chunks = Vec::new();
        let mut chunk_start = 0;
        loop {
            let remaining = len - chunk_start;
            if remaining <= 0 {
                break;
            }
            let chunk_len = min(STAGING_BUFFER_SIZE, remaining);
            let new_chunk = LazyChunk {
                loaded: false,
                data_len: chunk_len,
                data_offset: chunk_start,
            };
            chunk_start += chunk_len;
            chunks.push(new_chunk);
        }

        Self {
            data: vec![0u8; len],
            chunks,
        }
    }

    async fn as_slice<S: RangeBounds<usize>>(
        &mut self,
        bounds: S,
        data: &WgpuBufferMemoryBlock,
    ) -> &mut [u8] {
        // Download all needed slices
        // Calculate chunk indices
        let requested_start_byte_inclusive: usize = match bounds.start_bound() {
            Bound::Included(b) => *b,
            Bound::Excluded(b) => max(b - 1, 0),
            Bound::Unbounded => 0,
        };
        let requested_end_byte_inclusive: usize = match bounds.end_bound() {
            Bound::Included(b) => *b,
            Bound::Excluded(b) => max(b - 1, 0),
            Bound::Unbounded => self.data.len() - 1,
        };
        let start_chunk_inclusive = requested_start_byte_inclusive / STAGING_BUFFER_SIZE;
        let end_chunk_inclusive = requested_end_byte_inclusive / STAGING_BUFFER_SIZE;

        // Check we found the right chunks
        let start_byte_inclusive = self.chunks[start_chunk_inclusive].data_offset;
        let end_byte_exclusive = self.chunks[end_chunk_inclusive].data_offset
            + self.chunks[end_chunk_inclusive].data_len;
        assert!(start_byte_inclusive <= requested_start_byte_inclusive);
        assert!(end_byte_exclusive > end_byte_inclusive);

        // Get chunks
        let chunks = &mut self.chunks[start_chunk_inclusive..=end_chunk_inclusive];

        // Get mutable splits
        let (_, mut remaining) = self.data.split_at_mut(start_byte_inclusive);
        let mut slices_and_chunks = Vec::new();
        for chunk in chunks {
            let (slice, rhs) = remaining.split_at_mut(chunk.data_len);
            remaining = rhs;
            slices_and_chunks.push((slice, chunk));
        }

        // Load all async
        let futures = slices_and_chunks
            .into_iter()
            .map(|(slice, chunk)| chunk.download(slice, data));
        join_all(futures).await;

        return self.data.as_mut_slice();
    }

    async fn unload(&mut self, data: &WgpuBufferMemoryBlock) {
        let futures = self.chunks.iter_mut().map(|chunk| {
            let start = chunk.data_offset;
            let end = chunk.data_offset + chunk.data_len;
            let buffer = &self.data[start..end];
            chunk.upload(buffer, data)
        });

        join_all(futures).await;
    }
}

pub struct WgpuMappedMemoryBlock {
    data: WgpuBufferMemoryBlock,
    cpu_buffer: LazyBuffer,
}

#[async_trait]
impl MemoryBlock<WgpuBackend> for WgpuMappedMemoryBlock {
    async fn len(&self) -> usize {
        self.data.len
    }
}

#[async_trait]
impl MainMemoryBlock<WgpuBackend> for WgpuMappedMemoryBlock {
    async fn as_slice<S: RangeBounds<usize>>(&mut self, bounds: S) -> &mut [u8] {
        self.cpu_buffer.as_slice(bounds, &self.data)
    }

    async fn move_to_device_memory(self) -> WgpuUnmappedMemoryBlock {
        let WgpuMappedMemoryBlock {
            data,
            mut cpu_buffer,
        } = self;
        cpu_buffer.unload(&data).await;
        WgpuUnmappedMemoryBlock { data }
    }
}

pub struct WgpuUnmappedMemoryBlock {
    data: WgpuBufferMemoryBlock,
}

#[async_trait]
impl MemoryBlock<WgpuBackend> for WgpuUnmappedMemoryBlock {
    async fn len(&self) -> usize {
        self.data.len
    }
}

impl WgpuUnmappedMemoryBlock {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        upload_buffers: Arc<BufferRing>,
        download_buffers: Arc<BufferRing>,
        size: usize,
        label: String,
        initial_data: Option<&[u8]>,
    ) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(format!("GPU only accessible buffer [{}]", label).as_str()),
            size: size as BufferAddress,
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: initial_data.is_some(),
        });
        if let Some(initial_data) = initial_data {
            buffer
                .slice(..)
                .get_mapped_range_mut()
                .copy_from_slice(initial_data);
            buffer.unmap();
        }
        Self {
            data: WgpuBufferMemoryBlock {
                device,
                queue,
                upload_buffers,
                download_buffers,
                buffer,
                len: size,
            },
        }
    }
}

impl DeviceMemoryBlock<WgpuBackend> for WgpuUnmappedMemoryBlock {
    fn move_to_main_memory(self) -> WgpuMappedMemoryBlock {
        WgpuMappedMemoryBlock {
            cpu_buffer: LazyBuffer::new(self.data.len),
            data: self.data,
        }
    }
}
