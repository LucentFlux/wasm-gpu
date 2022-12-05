use crate::backend::lazy::{DeviceOnlyBuffer, Lazy, LazyBackend};
use crate::memory::{MainMemoryBlock, MemoryBlock};
use crate::typed::ToRange;
use crate::DeviceMemoryBlock;
use async_trait::async_trait;
use futures::future::join_all;
use perfect_derive::perfect_derive;
use std::alloc;
use std::alloc::Layout;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

/// Calculates the minimum x s.t. x is a multiple of alignment and x >= size
fn min_alignment_gt(size: usize, alignment: usize) -> usize {
    let padding = (alignment - (size % alignment)) % alignment;
    debug_assert!(padding < alignment); // minimum x
    let x = size + padding;
    debug_assert_eq!(x % alignment, 0); // x is a multiple of alignment
    debug_assert!(x >= size); // x >= size

    return x;
}

#[must_use]
#[perfect_derive(Debug)]
pub struct LazyBufferMemoryBlock<L: LazyBackend> {
    backend: Lazy<L>,
    pub buffer: L::DeviceOnlyBuffer, // Stored on the GPU
    visible_len: usize,
}

#[async_trait]
impl<L: LazyBackend> MemoryBlock<Lazy<L>> for LazyBufferMemoryBlock<L> {
    fn backend(&self) -> &Lazy<L> {
        &self.backend
    }

    fn len(&self) -> usize {
        self.visible_len
    }
}

/// Lazily downloaded chunks of data
#[perfect_derive(Debug)]
struct LazyChunk<L: LazyBackend> {
    initialized: AtomicBool,
    mutex: tokio::sync::Mutex<()>,
    data_offset: usize, // Offset within the GPU buffer
    dirty: AtomicBool,  // If the data chunk has possibly been written, we must write back
    _phantom: PhantomData<L>,
}

impl<L: LazyBackend> LazyChunk<L> {
    fn new(data_offset: usize) -> Self {
        Self {
            initialized: AtomicBool::from(false),
            dirty: AtomicBool::from(false),
            mutex: tokio::sync::Mutex::new(()),
            data_offset,
            _phantom: Default::default(),
        }
    }

    /// Lockless checking that we have definitely downloaded this data chunk from the GPU
    async fn ensure_downloaded(
        &self,
        block: &LazyBufferMemoryBlock<L>,
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
            .backend
            .download_buffers
            .with_slice(&block.buffer, self.data_offset, move |slice| {
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
                    std::slice::from_raw_parts_mut(offset_ptr, L::CHUNK_SIZE)
                };
                dest.copy_from_slice(slice);
            })
            .await;

        self.initialized.store(true, Ordering::Release);
    }

    /// Lockless checking that we have definitely uploaded this data (if it was dirty)
    async fn upload(&mut self, block: &LazyBufferMemoryBlock<L>, buffer: &HostMemoryBlob) {
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
        let src = buffer.as_slice(start..(start + L::CHUNK_SIZE));

        block
            .backend
            .upload_buffers
            .write_slice(&block.buffer, self.data_offset, src)
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
    fn new_layout(mut len: usize) -> Layout {
        if len == 0 {
            len = 1; // Single byte allocation shouldn't hurt anyone :)
        }
        Layout::array::<u8>(len).unwrap()
    }

    fn new(mut len: usize) -> Self {
        let layout = Self::new_layout(len);
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

    fn resize(&mut self, new_size: usize) {
        if new_size == self.len {
            return;
        }

        let new_ptr = unsafe { alloc::realloc(self.ptr, self.layout, new_size) };
        assert!(!new_ptr.is_null(), "failed to allocate blob");
        self.layout = Self::new_layout(new_size);
        self.ptr = new_ptr;
        self.len = new_size;
    }
}

impl Debug for HostMemoryBlob {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostMemoryBlob")
            .field("data", &self.as_slice(..))
            .finish()
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

#[perfect_derive(Debug)]
struct LazyBuffer<L: LazyBackend> {
    backing: LazyBufferMemoryBlock<L>,
    data: HostMemoryBlob,
    chunks: Vec<LazyChunk<L>>,
}

impl<L: LazyBackend> LazyBuffer<L> {
    fn new_chunk(i: usize) -> LazyChunk<L> {
        LazyChunk::new(i * L::CHUNK_SIZE)
    }

    pub fn new(backing: LazyBufferMemoryBlock<L>) -> Self {
        let alignment = L::CHUNK_SIZE;
        let full_len = backing.buffer.len();

        debug_assert_eq!(full_len % alignment, 0); // Exact chunks

        let mut chunks = Vec::new();
        chunks.reserve_exact(full_len / alignment);
        for i in 0..(full_len / alignment) {
            chunks.push(Self::new_chunk(i))
        }

        // Create an unmanaged buffer in memory
        let data = HostMemoryBlob::new(full_len);

        Self {
            backing,
            data,
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

        let start_chunk_inclusive = byte_bounds.start / L::CHUNK_SIZE;
        let end_chunk_inclusive = (byte_bounds.end - 1) / L::CHUNK_SIZE;

        // Check we found the right chunks
        let start_byte_inclusive = self.chunks[start_chunk_inclusive].data_offset as usize;
        let end_byte_exclusive =
            self.chunks[end_chunk_inclusive].data_offset as usize + L::CHUNK_SIZE;
        assert!(start_byte_inclusive <= byte_bounds.start);
        assert!(byte_bounds.end <= end_byte_exclusive);

        // Get chunks
        let chunks = &self.chunks[start_chunk_inclusive..=end_chunk_inclusive];

        // Load all async
        let futures = chunks
            .into_iter()
            .map(|chunk| chunk.ensure_downloaded(&self.backing, mark_as_dirty, &self.data));

        join_all(futures).await;
    }

    async fn as_slice<S: ToRange<usize>>(&self, bounds: S) -> &[u8] {
        let bounds = bounds.half_open(self.backing.visible_len);

        if bounds.start >= bounds.end {
            return &[];
        }

        assert!(bounds.start < self.backing.visible_len);
        assert!(bounds.end <= self.backing.visible_len);

        self.ensure_downloaded(bounds.clone(), false).await;

        return self.data.as_slice(bounds);
    }

    async fn as_slice_mut<S: ToRange<usize>>(&mut self, bounds: S) -> &mut [u8] {
        let bounds = bounds.half_open(self.backing.visible_len);

        if bounds.start >= bounds.end {
            return &mut [];
        }

        assert!(bounds.start < self.backing.visible_len);
        assert!(bounds.end <= self.backing.visible_len);

        self.ensure_downloaded(bounds.clone(), true).await;

        return self.data.as_slice_mut(bounds);
    }

    async fn unload(mut self) -> LazyBufferMemoryBlock<L> {
        let futures = self
            .chunks
            .iter_mut()
            .map(|chunk| chunk.upload(&self.backing, &self.data));

        join_all(futures).await;

        self.backing
    }

    async fn resize_inplace(&mut self, new_len: usize) {
        if new_len == self.backing.visible_len {
            return;
        }

        // Calculate chunks
        let new_real_size = min_alignment_gt(new_len, L::CHUNK_SIZE);
        let new_chunks_count = new_real_size / L::CHUNK_SIZE;

        // We may not need to change if we're within a chunk size still
        if new_real_size != self.backing.buffer.len() {
            // Create a new buffer and populate, without dropping any of our modified data
            let mut new_buffer = self
                .backing
                .backend
                .lazy
                .create_device_only_memory_block(new_len, None);
            new_buffer.copy_from(&self.backing.buffer).await;
        }

        let chunks_count = self.chunks.len();
        if new_chunks_count != chunks_count {
            // Shrink chunks
            for _ in new_chunks_count..chunks_count {
                let old = self
                    .chunks
                    .pop()
                    .expect("chunks was empty when it shouldn't be");
                drop(old)
            }
            // Grow chunks
            let chunks_count = self.chunks.len();
            for i in chunks_count..new_chunks_count {
                self.chunks.push(Self::new_chunk(i))
            }

            // Resize blob
            self.data.resize(new_real_size);
        }
    }
}

#[perfect_derive(Debug)]
pub struct MappedLazyBuffer<L: LazyBackend> {
    data: LazyBuffer<L>,
}

#[async_trait]
impl<L: LazyBackend> MemoryBlock<Lazy<L>> for MappedLazyBuffer<L> {
    fn backend(&self) -> &Lazy<L> {
        self.data.backing.backend()
    }

    fn len(&self) -> usize {
        self.data.backing.visible_len
    }
}

#[async_trait]
impl<L: LazyBackend> MainMemoryBlock<Lazy<L>> for MappedLazyBuffer<L> {
    async fn as_slice<S: ToRange<usize> + Send>(&self, bounds: S) -> &[u8] {
        self.data.as_slice(bounds).await
    }

    async fn as_slice_mut<S: ToRange<usize> + Send>(&mut self, bounds: S) -> &mut [u8] {
        self.data.as_slice_mut(bounds).await
    }

    async fn unmap(self) -> UnmappedLazyBuffer<L> {
        UnmappedLazyBuffer {
            data: self.data.unload().await,
        }
    }

    /// The `resize` method from this trait can be more efficiently re-implemented here to avoid
    /// flushing
    async fn resize(&mut self, new_len: usize) {
        self.data.resize_inplace(new_len).await;
    }
}

#[perfect_derive(Debug)]
pub struct UnmappedLazyBuffer<L: LazyBackend>
where
    Self: Send,
{
    pub data: LazyBufferMemoryBlock<L>,
}

impl<L: LazyBackend> UnmappedLazyBuffer<L> {
    pub fn new(
        backend: Lazy<L>,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> UnmappedLazyBuffer<L> {
        let real_size = min_alignment_gt(size, L::CHUNK_SIZE);

        // Pad initial data
        let mut initial_data = initial_data;
        let mut padded = vec![];
        if real_size != size {
            if let Some(v) = initial_data {
                assert_eq!(
                    v.len(),
                    size,
                    "initial data must match the visible length of the buffer"
                );
                padded = vec![0u8; real_size];
                padded.as_mut_slice()[0..size].copy_from_slice(v);
            }

            initial_data = initial_data.map(|_| padded.as_slice());
        }

        let buffer = <L as LazyBackend>::create_device_only_memory_block(
            &backend.lazy,
            real_size,
            initial_data,
        );
        Self {
            data: LazyBufferMemoryBlock {
                backend,
                buffer,
                visible_len: size,
            },
        }
    }
}

#[async_trait]
impl<L: LazyBackend> MemoryBlock<Lazy<L>> for UnmappedLazyBuffer<L> {
    fn backend(&self) -> &Lazy<L> {
        self.data.backend()
    }

    fn len(&self) -> usize {
        self.data.visible_len
    }
}

#[async_trait]
impl<L: LazyBackend> DeviceMemoryBlock<Lazy<L>> for UnmappedLazyBuffer<L> {
    async fn map(self) -> MappedLazyBuffer<L> {
        MappedLazyBuffer {
            data: LazyBuffer::new(self.data),
        }
    }

    async fn copy_from(&mut self, other: &UnmappedLazyBuffer<L>) {
        self.data.buffer.copy_from(&other.data.buffer).await
    }
}

#[cfg(test)]
mod tests {
    use crate::block_test;
    use crate::memory::DeviceMemoryBlock;
    use crate::tests_lib::{gen_test_data, get_backend};
    use crate::{Backend, MainMemoryBlock, MemoryBlock};
    use paste::paste;
    use tokio::runtime::Runtime;

    macro_rules! backend_buffer_tests {
        ($($value:expr),* $(,)?) => {
        $(
            block_test!($value, test_get_unmapped_len);
            block_test!($value, test_get_mapped_len);
            block_test!($value, test_upload_download);
            block_test!($value, test_write_read_mapped);
            block_test!($value, test_create_mapped_download);
        )*
        };
    }

    backend_buffer_tests!(0, 1, 7, 8, 9, 511, 512, 513, 1023, 1024, 1025, 4095, 4096, 4097);

    #[inline(never)]
    async fn test_get_unmapped_len(size: usize) {
        let backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);

        assert_eq!(memory.len(), size);
    }

    #[inline(never)]
    async fn test_get_mapped_len(size: usize) {
        let backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);
        let memory = memory.map().await;

        assert_eq!(memory.len(), size);
    }

    #[inline(never)]
    async fn test_create_mapped_download(size: usize) {
        let backend = get_backend().await;

        let expected_data = gen_test_data(size, (size * 33) as u32);

        let memory = backend.create_device_memory_block(size, Some(expected_data.as_slice()));

        // Read
        let memory = memory.map().await;
        let slice = memory.as_slice(..).await;
        let data_result = Vec::from(slice);

        assert_eq!(data_result, expected_data);
    }

    #[inline(never)]
    async fn test_write_read_mapped(size: usize) {
        let backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);
        let mut memory = memory.map().await;
        let slice = memory.as_slice_mut(..).await;

        // Write some data
        let expected_data = gen_test_data(size, size as u32);
        slice.copy_from_slice(expected_data.as_slice());

        // Read it back
        let slice = memory.as_slice(..).await;
        let data_result = Vec::from(slice);

        assert_eq!(data_result, expected_data);
    }

    #[inline(never)]
    async fn test_upload_download(size: usize) {
        let backend = get_backend().await;

        let memory = backend.create_device_memory_block(size, None);
        let mut memory = memory.map().await;
        let slice = memory.as_slice_mut(..).await;

        // Write some data
        let expected_data = gen_test_data(size, size as u32);
        slice.copy_from_slice(expected_data.as_slice());

        // Unmap and Remap
        let memory = memory.unmap().await;
        let memory = memory.map().await;
        let slice = memory.as_slice(..).await;
        let data_result = Vec::from(slice);

        assert_eq!(data_result, expected_data);
    }
}
