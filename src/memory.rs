//! Data can be either on the CPU or parallel computing device. Due to the existence of host functions,
//! we need to be able to map between these locations at runtime, with minimal overhead. This file deals with
//! these types

pub mod interleaved;

use crate::Backend;
use std::ops::RangeBounds;
use std::sync::{Arc, RwLock};

use crate::typed::ToRange;
use async_trait::async_trait;

#[async_trait]
pub trait MemoryBlock<B>
where
    B: Backend,
{
    async fn len(&self) -> usize;
}

#[async_trait]
pub trait MainMemoryBlock<B>: MemoryBlock<B>
where
    B: Backend,
{
    async fn as_slice<S: ToRange<usize> + Send>(&self, bounds: S) -> &[u8];
    async fn as_slice_mut<S: ToRange<usize> + Send>(&mut self, bounds: S) -> &mut [u8];
    async fn move_to_device_memory(self) -> B::DeviceMemoryBlock;
}

#[async_trait]
pub trait DeviceMemoryBlock<B>: MemoryBlock<B>
where
    B: Backend,
{
    async fn move_to_main_memory(self) -> B::MainMemoryBlock;
    async fn copy_from(&mut self, other: &B::DeviceMemoryBlock);
}

/// Used by MemoryBlock as a constant-size block of memory that tracks where it is
enum StaticMemoryBlock<B>
where
    B: Backend,
{
    Main(B::MainMemoryBlock),
    Device(B::DeviceMemoryBlock),
    None, // Used as an intermediate step for transformations to and from device. Invariant: Enum is never this
}

impl<B> StaticMemoryBlock<B>
where
    B: Backend,
{
    /// The current length of the block of memory, in bytes.
    pub async fn len(&self) -> usize {
        match self {
            Self::Main(m) => m.len().await,
            Self::Device(m) => m.len().await,
            Self::None => panic!("memory was lost"),
        }
    }

    async fn move_to_main(&mut self) {
        let mem: Self = std::mem::replace(self, Self::None);
        *self = Self::Main(match mem {
            Self::Main(m) => m,
            Self::Device(m) => m.move_to_main_memory().await,
            Self::None => panic!("memory was lost"),
        });
    }

    fn try_as_main(&self) -> Option<&B::MainMemoryBlock> {
        return match self {
            Self::Main(m) => Some(m),
            _ => None,
        };
    }

    async fn as_main(&mut self) -> &mut B::MainMemoryBlock {
        self.move_to_main().await;

        return match self {
            Self::Main(m) => m,
            _ => unreachable!(),
        };
    }

    async fn move_to_device(&mut self) {
        let mem: Self = std::mem::replace(self, Self::None);
        *self = Self::Device(match mem {
            Self::Device(m) => m,
            Self::Main(m) => m.move_to_device_memory().await,
            Self::None => panic!("memory was lost"),
        });
    }

    async fn as_device(&mut self) -> &mut B::DeviceMemoryBlock {
        self.move_to_device().await;

        return match self {
            Self::Device(m) => m,
            _ => unreachable!(),
        };
    }
}

/// Supports resizing via reallocation and copying
/// Used by DynamicMemoryBlock via a RwLock to provide lockless immutable slice access
struct DynamicMemoryBlockInternal<B>
where
    B: Backend,
{
    backend: Arc<B>,
    memory: StaticMemoryBlock<B>,
}

impl<B> DynamicMemoryBlockInternal<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>, size: usize, initial_data: Option<&[u8]>) -> Self {
        Self {
            backend,
            memory: StaticMemoryBlock::Device(
                backend.create_device_memory_block(size, initial_data),
            ),
        }
    }

    pub async fn len(&self) -> usize {
        self.memory.len().await
    }

    /// See [as_slice_mut](crate::Memory::as_slice_mut)
    pub async fn as_slice<S: ToRange<usize> + Send>(&mut self, bounds: S) -> &[u8] {
        let main_memory: &mut B::MainMemoryBlock = self.memory.as_main().await;

        return main_memory.as_slice(bounds).await;
    }

    /// See [as_slice_mut](crate::Memory::as_slice_mut)
    /// Returns None if the buffer is not already accessible. Fall back on as_slice
    pub async fn try_as_slice<S: ToRange<usize> + Send>(&self, bounds: S) -> Option<&[u8]> {
        let main_memory: Option<&B::MainMemoryBlock> = self.memory.try_as_main();

        match main_memory {
            None => None,
            Some(mem) => Some(mem.as_slice(bounds).await),
        }
    }

    pub async fn as_slice_mut<S: ToRange<usize> + Send>(&mut self, bounds: S) -> &mut [u8] {
        let mut main_memory: &mut B::MainMemoryBlock = self.memory.as_main().await;

        return main_memory.as_slice_mut(bounds).await;
    }

    pub async fn flush(&mut self) {
        self.memory.move_to_device().await;
    }

    pub async fn resize(&mut self, size: usize) {
        let mut new_buffer = self.backend.create_device_memory_block(size, None);
        {
            let old_buffer = self.memory.as_device().await;
            new_buffer.copy_from(old_buffer).await;
        }
        self.memory = StaticMemoryBlock::Device(new_buffer);
    }
}

/// Supports reading, writing and resizing, and semi-lockless (RwLock::read) access to read immutable slices
pub struct DynamicMemoryBlock<B>
where
    B: Backend,
{
    internal: RwLock<DynamicMemoryBlockInternal<B>>,
}

impl<B> DynamicMemoryBlock<B>
where
    B: Backend,
{
    pub fn new(backend: Arc<B>, size: usize, initial_data: Option<&[u8]>) -> Self {
        Self {
            internal: RwLock::new(DynamicMemoryBlockInternal::new(backend, size, initial_data)),
        }
    }

    pub async fn len(&self) -> usize {
        self.internal.read().unwrap().len().await
    }

    /// Maps the memory if needed. Aims to use a read lock, but falls back to write lock on the first
    /// time we need to map the buffer.
    /// Prefer over `as_slice_mut` to reduce memory transfers.
    ///
    /// Inside a host function evocation, memory is cached in ram. This means there is no performance
    /// hit to first read with `as_slice`, then write with `as_slice_mut`, rather than reading and
    /// writing with one `as_slice_mut` call. Prefer the former if possible, as it may reduce
    /// memory transfers.
    pub async fn as_slice<S: ToRange<usize> + Send + Clone>(&self, bounds: S) -> &[u8] {
        // Try lockless
        {
            let read_lock = self.internal.read().unwrap();
            let res = read_lock.try_as_slice(bounds.clone());
            if let Some(res) = res.await {
                return res;
            }
        }

        // Otherwise lock
        // Note we may race, but as_slice will just fall through if we did
        {
            let mut write_lock = self.internal.write().unwrap();
            let slice = write_lock.as_slice(bounds).await;
            return slice;
        }
    }

    /// Maps the memory if needed, and marks the entire slice as dirty and needing to be written back.
    /// Prefer `as_slice` to reduce memory transfers, and if you need mutability make your accesses as
    /// small as possible!
    ///
    /// Inside a host function evocation, memory is cached in ram. This means there is no performance
    /// hit to first read with `as_slice`, then write with `as_slice_mut`, rather than reading and
    /// writing with one `as_slice_mut` call. Prefer the former if possible, as it may reduce
    /// memory transfers.
    pub async fn as_slice_mut<S: ToRange<usize> + Send>(&mut self, bounds: S) -> &mut [u8] {
        return self.internal.get_mut().unwrap().as_slice_mut(bounds).await;
    }

    /// For internal use. Flush is automatically called after every host function, so there should be
    /// no reason for any uses of this library to call this function. It is exposed for future multithreaded
    /// wasm use cases, where host memory coherency calls may need to be more fine-grained.
    pub async fn flush(&mut self) {
        self.internal.get_mut().unwrap().flush().await;
    }

    pub async fn resize(&mut self, size: usize) {
        self.internal.get_mut().unwrap().resize(size).await;
    }

    /// Convenience wrapper around `resize` that adds more space
    pub async fn extend(&mut self, extra: usize) {
        let len = self.len().await;
        self.resize(len + extra).await
    }

    /// Convenience method for writing blocks of data
    pub async fn write(&mut self, data: &[u8], offset: usize) {
        let start = offset;
        let end = start + data.len();
        let slice = self.as_slice_mut(start..end).await;
        slice.copy_from_slice(data);
    }

    pub(crate) async fn as_device(&mut self) -> &mut B::DeviceMemoryBlock {
        return self.internal.get_mut().unwrap().memory.as_device().await;
    }
}

pub fn limits_match<V: Ord>(n1: V, m1: Option<V>, n2: V, m2: Option<V>) -> bool {
    if n1 > n2 {
        return false;
    }
    return match (m1, m2) {
        (None, None) => true,
        (Some(m1), Some(m2)) => (m1 >= m2),
        (_, _) => false,
    };
}
