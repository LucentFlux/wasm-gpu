use crate::memory::{DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};
use crate::wgpu::async_buffer::AsyncBuffer;
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::async_queue::AsyncQueue;
use crate::wgpu::buffer_ring::BufferRing;
use crate::wgpu::WgpuFuture;
use crate::WgpuBackend;
use async_trait::async_trait;
use futures::future::join_all;
use std::cmp;
use std::cmp::min;
use std::collections::Bound;
use std::ops::RangeBounds;
use std::sync::Arc;
use wgpu::{BufferAddress, BufferDescriptor, BufferUsages, MapMode};

/// Calculates the minimum x s.t. x is a multiple of wgpu::COPY_BUFFER_ALIGNMENT and x >= size
fn min_alignment_gt(size: usize) -> usize {
    let padding = (-(size as i64)).rem_euclid(wgpu::COPY_BUFFER_ALIGNMENT as i64) as usize;
    let new_size = size + padding;
    debug_assert_eq!(new_size % (wgpu::COPY_BUFFER_ALIGNMENT as usize), 0);
    debug_assert!(new_size >= size);

    return new_size;
}

/// Calculates the maximum x s.t. x is a multiple of wgpu::COPY_BUFFER_ALIGNMENT and x <= size
fn max_alignment_lt(size: usize) -> usize {
    let padding = size % (wgpu::COPY_BUFFER_ALIGNMENT as usize);
    debug_assert!(size >= padding);
    let new_size = size - padding;
    debug_assert_eq!(new_size % (wgpu::COPY_BUFFER_ALIGNMENT as usize), 0);
    debug_assert!(new_size >= size);

    return new_size;
}

struct WgpuBufferMemoryBlock {
    pub device: Arc<AsyncDevice>,
    pub queue: Arc<AsyncQueue>,
    pub upload_buffers: Arc<BufferRing>,
    pub download_buffers: Arc<BufferRing>,
    pub buffer: AsyncBuffer, // Stored on the GPU
    pub len: usize,
}

impl WgpuBufferMemoryBlock {
    fn buffer_size(&self) -> usize {
        assert_eq!(
            self.upload_buffers.buffer_size(),
            self.download_buffers.buffer_size()
        );

        return self.upload_buffers.buffer_size();
    }
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
    real_len: usize, // Multiple of wgpu::COPY_BUFFER_ALIGNMENT, geq to data_len
    data_offset: usize,
    dirty: bool, // If the data chunk has possibly been written, write back
}

impl LazyChunk {
    async fn download(
        &mut self,
        buffer: &mut [u8],
        block: &WgpuBufferMemoryBlock,
        mark_as_dirty: bool,
    ) {
        self.dirty |= mark_as_dirty;

        if self.loaded {
            return;
        }

        let download_buffer = block.download_buffers.pop().await;

        assert_eq!(buffer.len(), self.data_len);
        assert!(self.real_len >= self.data_len);
        assert!(self.real_len <= download_buffer.len());

        // Tell the GPU to move a chunk of data into the CPU accessible buffer
        let mut copy_command_encoder = block.device.create_command_encoder(&Default::default());
        copy_command_encoder.copy_buffer_to_buffer(
            block.buffer.as_ref(),
            self.data_offset as BufferAddress,
            download_buffer.buffer.as_ref(),
            0,
            self.real_len as BufferAddress,
        );
        block
            .queue
            .submit(vec![copy_command_encoder.finish()])
            .await;

        // Tell the GPU to, after the move, map the memory to the CPU
        {
            let slice = download_buffer
                .map_slice(..self.real_len as BufferAddress, MapMode::Read)
                .await
                .expect("failed to map download buffer");

            // Copy the data that the GPU gave us into our buffer in ram
            let view = slice.get_mapped_range();
            let view = &view.as_ref()[..self.data_len];
            buffer.copy_from_slice(view);
        }

        // Return our CPU accessible buffer
        block.download_buffers.push(download_buffer).await;

        self.loaded = true;
    }

    async fn upload(&mut self, buffer: &[u8], block: &WgpuBufferMemoryBlock) {
        if !self.loaded {
            return;
        }

        let upload_buffer = block.upload_buffers.pop().await;

        assert_eq!(buffer.len(), self.data_len);
        assert!(self.real_len >= self.data_len);
        assert!(self.real_len <= upload_buffer.len());

        // Copy data into GPU accessible buffer
        {
            let slice = upload_buffer.buffer.slice(..self.real_len as BufferAddress);
            let slice = &mut slice.get_mapped_range_mut()[..self.data_len];
            slice.copy_from_slice(buffer);
        }

        upload_buffer.buffer.unmap();

        // Tell GPU to copy into non-CPU accessible buffer
        let mut copy_command_encoder = block.device.create_command_encoder(&Default::default());
        copy_command_encoder.copy_buffer_to_buffer(
            upload_buffer.buffer.as_ref(),
            0,
            block.buffer.as_ref(),
            self.data_offset as BufferAddress,
            self.real_len as BufferAddress,
        );
        block
            .queue
            .submit(vec![copy_command_encoder.finish()])
            .await;

        // Return our CPU accessible buffer
        block.upload_buffers.push(upload_buffer).await;

        self.loaded = false;
    }
}

struct LazyBuffer {
    data: Vec<u8>,
    chunks: Vec<LazyChunk>,
}

impl LazyBuffer {
    pub fn new(len: usize, data: &WgpuBufferMemoryBlock) -> Self {
        let mut chunks = Vec::new();
        let mut chunk_start = 0;
        loop {
            let remaining = len - chunk_start;
            if remaining <= 0 {
                break;
            }
            let chunk_len = min(data.buffer_size(), remaining);
            let new_chunk = LazyChunk {
                loaded: false,
                data_len: chunk_len,
                real_len: min_alignment_gt(chunk_len),
                data_offset: chunk_start,
                dirty: false,
            };
            chunk_start += chunk_len;
            chunks.push(new_chunk);
        }

        Self {
            data: vec![0u8; len],
            chunks,
        }
    }

    fn start_bound_to_inclusive(b: Bound<&usize>) -> usize {
        match b {
            Bound::Included(b) => *b,
            Bound::Excluded(b) => *b + 1,
            Bound::Unbounded => 0,
        }
    }

    fn end_bound_to_exclusive(b: Bound<&usize>, end: usize) -> usize {
        match b {
            Bound::Included(b) => *b + 1,
            Bound::Excluded(b) => *b,
            Bound::Unbounded => end,
        }
    }

    async fn download_slices(
        &mut self,
        requested_start_byte_inclusive: usize,
        requested_end_byte_exclusive: usize,
        data: &WgpuBufferMemoryBlock,
        mark_as_dirty: bool,
    ) {
        // Download all needed slices
        assert!(
            requested_end_byte_exclusive <= self.data.len(),
            "end of buffer out of bounds"
        );

        if requested_end_byte_exclusive <= requested_start_byte_inclusive {
            return;
        }

        assert_ne!(requested_end_byte_exclusive, 0);
        let requested_end_byte_inclusive = requested_end_byte_exclusive - 1;

        let start_chunk_inclusive = requested_start_byte_inclusive / data.buffer_size();
        let end_chunk_inclusive = requested_end_byte_inclusive / data.buffer_size();

        // Check we found the right chunks
        let start_byte_inclusive = self.chunks[start_chunk_inclusive].data_offset;
        let end_byte_exclusive = self.chunks[end_chunk_inclusive].data_offset
            + self.chunks[end_chunk_inclusive].data_len;
        assert!(start_byte_inclusive <= requested_start_byte_inclusive);
        assert!(requested_end_byte_inclusive < end_byte_exclusive);

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
            .map(|(slice, chunk)| chunk.download(slice, data, mark_as_dirty));
        join_all(futures).await;
    }

    async fn as_slice<S: RangeBounds<usize>>(
        &mut self,
        bounds: S,
        data: &WgpuBufferMemoryBlock,
    ) -> anyhow::Result<&[u8]> {
        let requested_start_byte_inclusive: usize =
            Self::start_bound_to_inclusive(bounds.start_bound());
        let requested_end_byte_exclusive: usize =
            Self::end_bound_to_exclusive(bounds.end_bound(), data.len);

        self.download_slices(
            requested_start_byte_inclusive,
            requested_end_byte_exclusive,
            data,
            false,
        )
        .await;

        return Ok(&self.data[requested_start_byte_inclusive..requested_end_byte_exclusive]);
    }

    async fn as_slice_mut<'a, S: RangeBounds<usize>>(
        &'a mut self,
        bounds: S,
        data: &'a WgpuBufferMemoryBlock,
    ) -> anyhow::Result<&'a mut [u8]> {
        let requested_start_byte_inclusive: usize =
            Self::start_bound_to_inclusive(bounds.start_bound());
        let requested_end_byte_exclusive: usize =
            Self::end_bound_to_exclusive(bounds.end_bound(), data.len);

        self.download_slices(
            requested_start_byte_inclusive,
            requested_end_byte_exclusive,
            data,
            true,
        )
        .await;

        return Ok(&mut self.data[requested_start_byte_inclusive..requested_end_byte_exclusive]);
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
    async fn as_slice<S: RangeBounds<usize> + Send>(&mut self, bounds: S) -> anyhow::Result<&[u8]> {
        self.cpu_buffer.as_slice(bounds, &self.data).await
    }

    async fn as_slice_mut<S: RangeBounds<usize> + Send>(
        &mut self,
        bounds: S,
    ) -> anyhow::Result<&mut [u8]> {
        self.cpu_buffer.as_slice_mut(bounds, &self.data).await
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
        device: Arc<AsyncDevice>,
        queue: Arc<AsyncQueue>,
        upload_buffers: Arc<BufferRing>,
        download_buffers: Arc<BufferRing>,
        size: usize,
        label: String,
        initial_data: Option<&[u8]>,
    ) -> Self {
        let real_size = min_alignment_gt(size);

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(format!("GPU only accessible buffer [{}]", label).as_str()),
            size: real_size as BufferAddress,
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: initial_data.is_some(),
        });
        if let Some(initial_data) = initial_data {
            assert_eq!(initial_data.len(), size);

            buffer.slice(..).get_mapped_range_mut()[..initial_data.len()]
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

#[async_trait]
impl DeviceMemoryBlock<WgpuBackend> for WgpuUnmappedMemoryBlock {
    async fn move_to_main_memory(self) -> WgpuMappedMemoryBlock {
        WgpuMappedMemoryBlock {
            cpu_buffer: LazyBuffer::new(self.data.len, &self.data),
            data: self.data,
        }
    }

    async fn copy_from(&mut self, other: &WgpuUnmappedMemoryBlock) {
        // Tell GPU to copy from other into this
        let mut copy_command_encoder = self.data.device.create_command_encoder(&Default::default());
        copy_command_encoder.copy_buffer_to_buffer(
            other.data.buffer.as_ref(),
            0,
            self.data.buffer.as_ref(),
            0 as BufferAddress,
            min(self.data.len, other.data.len) as BufferAddress,
        );
        self.data
            .queue
            .submit(vec![copy_command_encoder.finish()])
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_test;
    use crate::tests_lib::{gen_test_data, get_backend};
    use crate::{
        wasp, Backend, BufferRingConfig, DeviceMemoryBlock, MainMemoryBlock, MemoryBlock,
        WgpuBackend, WgpuBackendConfig,
    };
    use paste::paste;
    use tokio::runtime::Runtime;

    macro_rules! backend_buffer_tests {
        ($($value:expr,)*) => {
        $(
            block_test!($value, test_get_unmapped_len);
            block_test!($value, test_get_mapped_len);
            block_test!($value, test_upload_download);
            block_test!($value, test_create_mapped_download);
        )*
        };
    }

    backend_buffer_tests!(
        0, 1, 7, 8, 9, 1023, 1024, 1025, 1048575, //(1024 * 1024 - 1),
        1048576, //(1024 * 1024),
        1048577, //(1024 * 1024 + 1),
    );

    #[inline(never)]
    async fn test_get_unmapped_len(size: usize) {
        let mut backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);

        assert_eq!(memory.len().await, size);
    }

    #[inline(never)]
    async fn test_get_mapped_len(size: usize) {
        let mut backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);
        let memory = memory.move_to_main_memory().await;

        assert_eq!(memory.len().await, size);
    }

    #[inline(never)]
    async fn test_create_mapped_download(size: usize) {
        let mut backend = get_backend().await;

        let expected_data = gen_test_data(size, (size * 33) as u32);

        let memory = backend.create_device_memory_block(size, Some(expected_data.as_slice()));

        // Read
        let mut memory = memory.move_to_main_memory().await;
        let slice = memory.as_slice(..).await.expect("could not map memory");
        let data_result = Vec::from(slice);

        assert_eq!(data_result, expected_data);
    }

    #[inline(never)]
    async fn test_upload_download(size: usize) {
        let mut backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);
        let mut memory = memory.move_to_main_memory().await;
        let slice = memory.as_slice_mut(..).await.expect("could not map memory");

        // Write some data
        let expected_data = gen_test_data(size, size as u32);
        slice.copy_from_slice(expected_data.as_slice());

        // Unmap and Remap
        let memory = memory.move_to_device_memory().await;
        let mut memory = memory.move_to_main_memory().await;
        let slice = memory.as_slice(..).await.expect("could not re-map memory");
        let data_result = Vec::from(slice);

        assert_eq!(data_result, expected_data);
    }
}
