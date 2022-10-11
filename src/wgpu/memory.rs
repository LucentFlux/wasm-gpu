use crate::memory::{DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};
use crate::wgpu::async_buffer::AsyncBuffer;
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::async_queue::AsyncQueue;
use crate::wgpu::buffer_ring::BufferRing;
use crate::WgpuBackend;
use async_trait::async_trait;
use futures::future::join_all;
use itertools::Itertools;
use std::cmp::min;
use std::collections::Bound;
use std::ops::RangeBounds;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use wgpu::{BufferAddress, BufferDescriptor, BufferUsages};

/// Calculates the minimum x s.t. x is a multiple of alignment and x >= size
fn min_alignment_gt(size: usize, alignment: usize) -> usize {
    let padding = (alignment - (size % alignment)) % alignment;
    debug_assert!(padding < alignment); // minimum x
    let x = size + padding;
    debug_assert_eq!(x % alignment, 0); // x is a multiple of alignment
    debug_assert!(x >= size); // x >= size

    return x;
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
    fn buffer_ring_buffer_size(&self) -> usize {
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
    device: Arc<AsyncDevice>,
    queue: Arc<AsyncQueue>,

    initialized: AtomicBool,
    mutex: Mutex<()>,
    data_offset: BufferAddress, // Offset within the GPU buffer
    ptr: *mut u8, // Pointer within the CPU buffer, len equal to self.block.download_buffers.buffer_size()
    dirty: AtomicBool, // If the data chunk has possibly been written, we must write back
}

impl LazyChunk {
    fn new(
        device: Arc<AsyncDevice>,
        queue: Arc<AsyncQueue>,
        ptr: *mut u8,
        data_offset: usize,
    ) -> Self {
        Self {
            device,
            queue,
            initialized: AtomicBool::from(false),
            dirty: AtomicBool::from(false),
            mutex: Mutex::new(()),
            ptr,
            data_offset: data_offset as BufferAddress,
        }
    }

    /// Lockless checking that we have definitely downloaded this data chunk from the GPU
    async fn ensure_downloaded(&self, block: &WgpuBufferMemoryBlock, mark_as_dirty: bool) {
        if mark_as_dirty {
            self.dirty.store(true, Ordering::Release);
        }

        // (http://schd.ws/hosted_files/cppcon2016/74/HansWeakAtomics.pdf Page 27)
        if self.initialized.load(Ordering::Acquire) {
            return;
        }

        // We *may* not be initialized. We have to block to be certain.
        let _internal = self.mutex.lock().unwrap();
        if self.initialized.load(Ordering::Relaxed) {
            // We raced, and someone else initialized us. We can fall
            // through now.
            return;
        }

        block.download_buffers.with_slice(
            &self.device,
            &self.queue,
            &block.buffer,
            self.data_offset,
            |slice| {
                // Assume pointers don't alias between chunks, and we have a mutex taken out for this chunk
                // So this should be safe
                let buffer = unsafe { std::slice::from_raw_parts_mut(self.ptr, slice.len()) };
                buffer.copy_from_slice(slice);
            },
        );

        self.initialized.store(true, Ordering::Release);
    }

    /// Lockless checking that we have definitely uploaded this data (if it was dirty)
    async fn upload(&self, block: &WgpuBufferMemoryBlock) {
        // (http://schd.ws/hosted_files/cppcon2016/74/HansWeakAtomics.pdf Page 27)
        if !self.initialized.load(Ordering::Acquire) || !self.dirty.load(Ordering::Acquire) {
            return;
        }

        // We *may* not be initialized. We have to block to be certain.
        let _internal = self.mutex.lock().unwrap();
        if !self.initialized.load(Ordering::Relaxed) || !self.dirty.load(Ordering::Relaxed) {
            // We raced, and someone else uploaded us. We can fall
            // through now.
            return;
        }

        // Assume pointers don't alias between chunks, and we have a mutex taken out for this chunk
        // So this should be safe
        let len = block.upload_buffers.buffer_size();
        let slice = unsafe { std::slice::from_raw_parts(self.ptr, len) };
        block
            .upload_buffers
            .write_slice(
                &self.device,
                &self.queue,
                &block.buffer,
                self.data_offset,
                slice,
            )
            .await;

        self.dirty.store(false, Ordering::Release);
    }
}

struct LazyBuffer {
    buffer: WgpuBufferMemoryBlock,
    data: Vec<u8>,
    len: usize,
    chunks: Vec<LazyChunk>, // Holds pointers into above data buffer. Pointers must not alias
}

impl LazyBuffer {
    pub fn new(len: usize, buffer: WgpuBufferMemoryBlock) -> Self {
        let alignment = buffer.buffer_ring_buffer_size();
        let full_len = min_alignment_gt(len, alignment);

        let data = vec![0u8; full_len];

        let mut iter = data.chunks_exact(alignment);

        assert_eq!(iter.remainder().len(), 0); // Exact chunks

        let mut chunks = Vec::new();
        chunks.reserve_exact(full_len / alignment);
        for (i, chunk) in iter.enumerate() {
            chunks.push(LazyChunk::new(
                buffer.device.clone(),
                buffer.queue.clone(),
                chunk.as_ptr() as *mut u8,
                i * alignment,
            ))
        }

        Self {
            buffer,
            data,
            len,
            chunks,
        }
    }

    /// Lockless checking that we have definitely downloaded all required data from the GPU to our buffer
    async fn ensure_downloaded(
        &self,
        requested_start_byte_inclusive: usize,
        requested_end_byte_exclusive: usize,
        mark_as_dirty: bool,
    ) {
        assert!(
            requested_end_byte_exclusive <= self.data.len(),
            "end of buffer out of bounds"
        );

        if requested_end_byte_exclusive <= requested_start_byte_inclusive {
            return;
        }

        assert_ne!(requested_end_byte_exclusive, 0);
        let requested_end_byte_inclusive = requested_end_byte_exclusive - 1;

        let alignment = self.buffer.buffer_ring_buffer_size();

        let start_chunk_inclusive = requested_start_byte_inclusive / alignment;
        let end_chunk_inclusive = requested_end_byte_inclusive / alignment;

        // Check we found the right chunks
        let start_byte_inclusive = self.chunks[start_chunk_inclusive].data_offset as usize;
        let end_byte_exclusive = self.chunks[end_chunk_inclusive].data_offset as usize + alignment;
        assert!(start_byte_inclusive <= requested_start_byte_inclusive);
        assert!(requested_end_byte_inclusive < end_byte_exclusive);

        // Get chunks
        let chunks = &self.chunks[start_chunk_inclusive..=end_chunk_inclusive];

        // Load all async
        let futures = chunks
            .into_iter()
            .map(|chunk| chunk.ensure_downloaded(&self.buffer, mark_as_dirty));
        join_all(futures).await;
    }

    async fn as_slice<S: RangeBounds<usize>>(&self, bounds: S) -> &[u8] {
        let requested_start_byte_inclusive: usize =
            Self::start_bound_to_inclusive(bounds.start_bound());
        let requested_end_byte_exclusive: usize =
            Self::end_bound_to_exclusive(bounds.end_bound(), self.len);

        assert!(requested_start_byte_inclusive < self.len);
        assert!(requested_end_byte_exclusive <= self.len);

        self.ensure_downloaded(
            requested_start_byte_inclusive,
            requested_end_byte_exclusive,
            false,
        )
        .await;

        return &self.data[requested_start_byte_inclusive..requested_end_byte_exclusive];
    }

    async fn as_slice_mut<S: RangeBounds<usize>>(&mut self, bounds: S) -> &mut [u8] {
        let requested_start_byte_inclusive: usize =
            Self::start_bound_to_inclusive(bounds.start_bound());
        let requested_end_byte_exclusive: usize =
            Self::end_bound_to_exclusive(bounds.end_bound(), self.len);

        assert!(requested_start_byte_inclusive < self.len);
        assert!(requested_end_byte_exclusive <= self.len);

        self.ensure_downloaded(
            requested_start_byte_inclusive,
            requested_end_byte_exclusive,
            true,
        )
        .await;

        return &mut self.data[requested_start_byte_inclusive..requested_end_byte_exclusive];
    }

    async fn unload(mut self) -> WgpuBufferMemoryBlock {
        let futures = self
            .chunks
            .iter_mut()
            .map(|chunk| chunk.upload(&self.buffer));

        join_all(futures).await;

        self.buffer
    }
}

pub struct WgpuMappedMemoryBlock {
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
    async fn as_slice<S: RangeBounds<usize> + Send>(&self, bounds: S) -> &[u8] {
        self.cpu_buffer.as_slice(bounds).await
    }

    async fn as_slice_mut<S: RangeBounds<usize> + Send>(&mut self, bounds: S) -> &mut [u8] {
        self.cpu_buffer.as_slice_mut(bounds).await
    }

    async fn move_to_device_memory(self) -> WgpuUnmappedMemoryBlock {
        let data = self.cpu_buffer.unload().await;
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
        assert_eq!(upload_buffers.buffer_size(), download_buffers.buffer_size());

        let real_size = min_alignment_gt(size, upload_buffers.buffer_size());

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
            cpu_buffer: LazyBuffer::new(self.data.len, self.data),
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
    use crate::block_test;
    use crate::tests_lib::{gen_test_data, get_backend};
    use crate::{Backend, DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};
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
