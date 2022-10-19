use crate::memory::{DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};
use crate::typed::ToRange;
use crate::wgpu::async_buffer::AsyncBuffer;
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::async_queue::AsyncQueue;
use crate::wgpu::buffer_ring::{BufferRing, ConstMode};
use crate::WgpuBackend;
use async_trait::async_trait;
use futures::future::join_all;
use std::alloc;
use std::alloc::Layout;
use std::cmp::min;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

struct WgpuBufferMemoryBlock<const BUFFER_SIZE: usize> {
    backend: Arc<WgpuBackend<BUFFER_SIZE>>,
    pub device: AsyncDevice,
    pub queue: AsyncQueue,
    pub upload_buffers: Arc<BufferRing<BUFFER_SIZE, { ConstMode::Write }>>,
    pub download_buffers: Arc<BufferRing<BUFFER_SIZE, { ConstMode::Read }>>,
    pub buffer: AsyncBuffer, // Stored on the GPU
    pub len: usize,
}

impl<const BUFFER_SIZE: usize> WgpuBufferMemoryBlock<BUFFER_SIZE> {
    fn buffer_ring_buffer_size(&self) -> usize {
        assert_eq!(
            self.upload_buffers.buffer_size(),
            self.download_buffers.buffer_size()
        );

        return self.upload_buffers.buffer_size();
    }
}

#[async_trait]
impl<const BUFFER_SIZE: usize> MemoryBlock<WgpuBackend<BUFFER_SIZE>>
    for WgpuBufferMemoryBlock<BUFFER_SIZE>
{
    fn backend(&self) -> &WgpuBackend<BUFFER_SIZE> {
        self.backend.as_ref()
    }

    async fn len(&self) -> usize {
        self.len
    }
}

/// Lazily downloaded chunks of data
struct LazyChunk<const SIZE: usize> {
    device: AsyncDevice,
    queue: AsyncQueue,

    initialized: AtomicBool,
    mutex: tokio::sync::Mutex<()>,
    data_offset: BufferAddress, // Offset within the GPU buffer
    dirty: AtomicBool,          // If the data chunk has possibly been written, we must write back
}

impl<const SIZE: usize> LazyChunk<SIZE> {
    fn new(device: AsyncDevice, queue: AsyncQueue, data_offset: usize) -> Self {
        Self {
            device,
            queue,
            initialized: AtomicBool::from(false),
            dirty: AtomicBool::from(false),
            mutex: tokio::sync::Mutex::new(()),
            data_offset: data_offset as BufferAddress,
        }
    }

    /// Lockless checking that we have definitely downloaded this data chunk from the GPU
    async fn ensure_downloaded(
        &self,
        block: &WgpuBufferMemoryBlock<SIZE>,
        mark_as_dirty: bool,
        buffer: &HostMemoryBlob,
    ) {
        if mark_as_dirty {
            self.dirty.store(true, Ordering::Release);
        }

        // (http://schd.ws/hosted_files/cppcon2016/74/HansWeakAtomics.pdf Page 27)
        if self.initialized.load(Ordering::Acquire) {
            return;
        }

        // We *may* not be initialized. We have to block to be certain.
        let _internal = self.mutex.lock().await;
        if self.initialized.load(Ordering::Relaxed) {
            // We raced, and someone else initialized us. We can fall
            // through now.
            return;
        }

        block
            .download_buffers
            .with_slice(
                &self.device,
                &self.queue,
                &block.buffer,
                self.data_offset,
                move |slice| {
                    // Safety proof:
                    // - we are uninitialized, so no references to the buffer
                    //   should exist outside of this function
                    // - we have a mutex out for this chunk, so we aren't racing
                    // - no other chunk overlaps with us
                    // Therefore we are the only code accessing this block of bytes
                    //
                    // If a chunk within the bounds of the blob is uninitialized,
                    // that means that no reference to that chunk's slice exists,
                    // as the first time a reference is created is the only time
                    // that a chunk is initialized. Therefore, with a mutex taken out,
                    // it is safe to get a mutable pointer to fill the initial chunk data.
                    let dest = unsafe {
                        let offset_ptr = buffer.ptr.add(self.data_offset as usize);
                        std::slice::from_raw_parts_mut(offset_ptr, SIZE)
                    };
                    dest.copy_from_slice(slice);
                },
            )
            .await;

        self.initialized.store(true, Ordering::Release);
    }

    /// Lockless checking that we have definitely uploaded this data (if it was dirty)
    async fn upload(&mut self, block: &WgpuBufferMemoryBlock<SIZE>, buffer: &HostMemoryBlob) {
        // (http://schd.ws/hosted_files/cppcon2016/74/HansWeakAtomics.pdf Page 27)
        if !self.initialized.load(Ordering::Acquire) || !self.dirty.load(Ordering::Acquire) {
            return;
        }

        // We *may* not be initialized. We have to block to be certain.
        let _internal = self.mutex.lock().await;
        if !self.initialized.load(Ordering::Relaxed) || !self.dirty.load(Ordering::Relaxed) {
            // We raced, and someone else uploaded us. We can fall
            // through now.
            return;
        }

        let start = self.data_offset as usize;
        let src = buffer.as_slice(start..(start + SIZE));

        block
            .upload_buffers
            .write_slice(
                &self.device,
                &self.queue,
                &block.buffer,
                self.data_offset,
                src.try_into().unwrap(), // We took a slice of length SIZE, so this should never fail
            )
            .await;

        self.dirty.store(false, Ordering::Release);
    }
}

/// We want lockless lazy initialization of segments of a buffer - this is too much for Rust!
/// This struct is essentially a `Box<[u8]>`, except we have access to pointer arithmetic
/// The only time that we should need to avoid rust's borrow checker is when we get a mutable slice
/// at chunk initialization time. See there for a safety argument
struct HostMemoryBlob {
    ptr: *mut u8,
    len: usize,
    layout: Layout,
}

impl HostMemoryBlob {
    fn new(len: usize) -> Self {
        let layout = Layout::array::<u8>(len).unwrap();
        let ptr = unsafe { alloc::alloc(layout) };
        Self { ptr, len, layout }
    }

    fn as_slice<S: ToRange<usize>>(&self, bounds: S) -> &[u8] {
        let bounds = bounds.half_open(self.len);
        assert!(bounds.end <= self.len);

        if bounds.end <= bounds.start {
            return &[];
        }

        let slice = unsafe {
            // Safety proof:
            //  `bounds.end <= self.len`
            //  `bounds.end - bounds.start = bounds.len()`
            // therefore
            //  `self.ptr + bounds.start + bounds.len() == self.ptr + bounds.end`
            // and so
            //  `self.ptr + bounds.start + bounds.len() <= self.ptr + self.len`
            // Since the buffer held by `self.ptr` is valid to dereference
            // up to `self.ptr + self.len` then this is fine
            let ptr = self.ptr.add(bounds.start);
            // Safety proof:
            // - this function takes an immutable reference
            // - all other unsafe blocks should ensure they don't violate rust's safety rules
            // - therefore the rust borrow checker should validate this immutable access
            std::slice::from_raw_parts(ptr, bounds.len())
        };

        return slice;
    }

    fn as_slice_mut<S: ToRange<usize>>(&mut self, bounds: S) -> &mut [u8] {
        let bounds = bounds.half_open(self.len);
        assert!(bounds.end <= self.len);

        if bounds.end <= bounds.start {
            return &mut [];
        }

        // Safety proof:
        // see immutable variation above
        let slice = unsafe {
            let ptr = self.ptr.add(bounds.start);
            std::slice::from_raw_parts_mut(ptr, bounds.len())
        };

        return slice;
    }
}

impl Drop for HostMemoryBlob {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(self.ptr, self.layout);
        }
    }
}

// Steal Box's rules
unsafe impl Send for HostMemoryBlob where Box<[u8]>: Send {}
unsafe impl Sync for HostMemoryBlob where Box<[u8]>: Sync {}

struct LazyBuffer<const BUFFER_SIZE: usize> {
    buffer: WgpuBufferMemoryBlock<BUFFER_SIZE>,
    data: HostMemoryBlob,
    len: usize,
    chunks: Vec<LazyChunk<BUFFER_SIZE>>,
}

impl<const BUFFER_SIZE: usize> LazyBuffer<BUFFER_SIZE> {
    pub fn new(len: usize, buffer: WgpuBufferMemoryBlock<BUFFER_SIZE>) -> Self {
        let alignment = buffer.buffer_ring_buffer_size();
        let full_len = min_alignment_gt(len, alignment);

        debug_assert_eq!(full_len % alignment, 0); // Exact chunks

        let mut chunks = Vec::new();
        chunks.reserve_exact(full_len / alignment);
        for i in 0..(full_len / alignment) {
            chunks.push(LazyChunk::new(
                buffer.device.clone(),
                buffer.queue.clone(),
                i * alignment,
            ))
        }

        // Create an unmanaged buffer in memory
        let data = HostMemoryBlob::new(full_len);

        Self {
            buffer,
            data,
            len,
            chunks,
        }
    }

    /// Lockless checking that we have definitely downloaded all required data from the GPU to our buffer
    async fn ensure_downloaded<S: ToRange<usize>>(&self, byte_bounds: S, mark_as_dirty: bool) {
        let byte_bounds = byte_bounds.half_open(self.data.len);

        assert!(
            byte_bounds.end <= self.data.len,
            "end of buffer out of bounds"
        );

        if byte_bounds.end <= byte_bounds.start {
            return;
        }

        assert_ne!(byte_bounds.end, 0);

        let start_chunk_inclusive = byte_bounds.start / BUFFER_SIZE;
        let end_chunk_inclusive = (byte_bounds.end - 1) / BUFFER_SIZE;

        // Check we found the right chunks
        let start_byte_inclusive = self.chunks[start_chunk_inclusive].data_offset as usize;
        let end_byte_exclusive =
            self.chunks[end_chunk_inclusive].data_offset as usize + BUFFER_SIZE;
        assert!(start_byte_inclusive <= byte_bounds.start);
        assert!(byte_bounds.end <= end_byte_exclusive);

        // Get chunks
        let chunks = &self.chunks[start_chunk_inclusive..=end_chunk_inclusive];

        // Load all async
        let futures = chunks
            .into_iter()
            .map(|chunk| chunk.ensure_downloaded(&self.buffer, mark_as_dirty, &self.data));
        join_all(futures).await;
    }

    async fn as_slice<S: ToRange<usize>>(&self, bounds: S) -> &[u8] {
        let bounds = bounds.half_open(self.len);

        assert!(bounds.start < self.len);
        assert!(bounds.end <= self.len);

        self.ensure_downloaded(bounds.clone(), false).await;

        return self.data.as_slice(bounds);
    }

    async fn as_slice_mut<S: ToRange<usize>>(&mut self, bounds: S) -> &mut [u8] {
        let bounds = bounds.half_open(self.len);

        assert!(bounds.start < self.len);
        assert!(bounds.end <= self.len);

        self.ensure_downloaded(bounds.clone(), true).await;

        return self.data.as_slice_mut(bounds);
    }

    async fn unload(mut self) -> WgpuBufferMemoryBlock<BUFFER_SIZE> {
        let futures = self
            .chunks
            .iter_mut()
            .map(|chunk| chunk.upload(&self.buffer, &self.data));

        join_all(futures).await;

        self.buffer
    }
}

pub struct WgpuMappedMemoryBlock<const BUFFER_SIZE: usize> {
    cpu_buffer: LazyBuffer<BUFFER_SIZE>,
}

#[async_trait]
impl<const BUFFER_SIZE: usize> MemoryBlock<WgpuBackend<BUFFER_SIZE>>
    for WgpuMappedMemoryBlock<BUFFER_SIZE>
{
    fn backend(&self) -> &WgpuBackend<BUFFER_SIZE> {
        self.cpu_buffer.buffer.backend()
    }

    async fn len(&self) -> usize {
        self.cpu_buffer.len
    }
}

#[async_trait]
impl<const BUFFER_SIZE: usize> MainMemoryBlock<WgpuBackend<BUFFER_SIZE>>
    for WgpuMappedMemoryBlock<BUFFER_SIZE>
{
    async fn as_slice<S: ToRange<usize> + Send>(&self, bounds: S) -> &[u8] {
        self.cpu_buffer.as_slice(bounds).await
    }

    async fn as_slice_mut<S: ToRange<usize> + Send>(&mut self, bounds: S) -> &mut [u8] {
        self.cpu_buffer.as_slice_mut(bounds).await
    }

    async fn move_to_device_memory(self) -> WgpuUnmappedMemoryBlock<BUFFER_SIZE> {
        let data = self.cpu_buffer.unload().await;
        WgpuUnmappedMemoryBlock { data }
    }
}

pub struct WgpuUnmappedMemoryBlock<const BUFFER_SIZE: usize> {
    data: WgpuBufferMemoryBlock<BUFFER_SIZE>,
}

#[async_trait]
impl<const BUFFER_SIZE: usize> MemoryBlock<WgpuBackend<BUFFER_SIZE>>
    for WgpuUnmappedMemoryBlock<BUFFER_SIZE>
{
    fn backend(&self) -> &WgpuBackend<BUFFER_SIZE> {
        self.data.backend()
    }

    async fn len(&self) -> usize {
        self.data.len
    }
}

impl<const BUFFER_SIZE: usize> WgpuUnmappedMemoryBlock<BUFFER_SIZE> {
    pub fn new(
        backend: Arc<WgpuBackend<BUFFER_SIZE>>,
        device: AsyncDevice,
        queue: AsyncQueue,
        upload_buffers: Arc<BufferRing<BUFFER_SIZE, { ConstMode::Write }>>,
        download_buffers: Arc<BufferRing<BUFFER_SIZE, { ConstMode::Read }>>,
        size: usize,
        label: String,
        initial_data: Option<&[u8]>,
    ) -> Self {
        assert_eq!(upload_buffers.buffer_size(), download_buffers.buffer_size());

        let real_size = min_alignment_gt(size, upload_buffers.buffer_size());

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(format!("GPU only accessible buffer [{}]", label).as_str()),
            size: real_size as BufferAddress,
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST | BufferUsages::STORAGE,
            mapped_at_creation: initial_data.is_some(),
        });
        if let Some(initial_data) = initial_data {
            assert_eq!(initial_data.len(), size);

            buffer.as_ref().slice(..).get_mapped_range_mut()[..initial_data.len()]
                .copy_from_slice(initial_data);
            buffer.as_ref().unmap();
        }
        Self {
            data: WgpuBufferMemoryBlock {
                backend,
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
impl<const BUFFER_SIZE: usize> DeviceMemoryBlock<WgpuBackend<BUFFER_SIZE>>
    for WgpuUnmappedMemoryBlock<BUFFER_SIZE>
{
    async fn move_to_main_memory(self) -> WgpuMappedMemoryBlock<BUFFER_SIZE> {
        WgpuMappedMemoryBlock {
            cpu_buffer: LazyBuffer::new(self.data.len, self.data),
        }
    }

    async fn copy_from(&mut self, other: &WgpuUnmappedMemoryBlock<BUFFER_SIZE>) {
        // Tell GPU to copy from other into this
        let mut copy_command_encoder = self
            .data
            .device
            .as_ref()
            .create_command_encoder(&Default::default());
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
        let backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);

        assert_eq!(memory.len().await, size);
    }

    #[inline(never)]
    async fn test_get_mapped_len(size: usize) {
        let backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);
        let memory = memory.move_to_main_memory().await;

        assert_eq!(memory.len().await, size);
    }

    #[inline(never)]
    async fn test_create_mapped_download(size: usize) {
        let backend = get_backend().await;

        let expected_data = gen_test_data(size, (size * 33) as u32);

        let memory = backend.create_device_memory_block(size, Some(expected_data.as_slice()));

        // Read
        let memory = memory.move_to_main_memory().await;
        let slice = memory.as_slice(..).await;
        let data_result = Vec::from(slice);

        assert_eq!(data_result, expected_data);
    }

    #[inline(never)]
    async fn test_upload_download(size: usize) {
        let backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);
        let mut memory = memory.move_to_main_memory().await;
        let slice = memory.as_slice_mut(..).await;

        // Write some data
        let expected_data = gen_test_data(size, size as u32);
        slice.copy_from_slice(expected_data.as_slice());

        // Unmap and Remap
        let memory = memory.move_to_device_memory().await;
        let memory = memory.move_to_main_memory().await;
        let slice = memory.as_slice(..).await;
        let data_result = Vec::from(slice);

        assert_eq!(data_result, expected_data);
    }
}
