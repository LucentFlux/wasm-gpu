//! To implement a backend, only the traits in `crate::backend` are required to be implemented.
//! However GPUs are GPUs so there is often a lot of shared methodology. This module offers an
//! alternate trait to implement to provide a lazy backend that copies data to and from the GPU
//! only when needed. Note that this being good assumes that the GPU and CPU have separate memories,
//! and that GPU memory can be mapped to main memory, and that this mapping is slower than
//! unmappable buffers.

use crate::backend::lazy::buffer_ring::read::ReadBufferRing;
use crate::backend::lazy::buffer_ring::write::WriteBufferRing;
use crate::backend::lazy::buffer_ring::BufferRingConfig;
use crate::backend::lazy::memory::{MappedLazyBuffer, UnmappedLazyBuffer};
use crate::Backend;
use async_trait::async_trait;
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

pub mod buffer_ring;
pub mod memory;

// The new lazy API
#[async_trait]
pub trait DeviceToMainBufferUnmapped<L: LazyBackend> {
    type Error: Error;

    async fn copy_from_and_map(
        self,
        src: &L::DeviceOnlyBuffer,
        offset: usize,
    ) -> Result<L::DeviceToMainBufferMapped, Self::Error>;
}
#[async_trait]
pub trait DeviceToMainBufferMapped<L: LazyBackend> {
    type Error: Error;

    type Dirty: DeviceToMainBufferDirty<L> + Send + Sync; // Allows for some cleaning to be done off-thread after a buffer has been used

    fn view_and_finish<Res, F: FnOnce(&[u8]) -> Res>(
        self,
        callback: F,
    ) -> Result<(Res, Self::Dirty), Self::Error>;
}
#[async_trait]
pub trait DeviceToMainBufferDirty<L: LazyBackend> {
    type Error: Error;

    async fn clean(self) -> Result<L::DeviceToMainBufferUnmapped, Self::Error>;
}

#[async_trait]
pub trait MainToDeviceBufferUnmapped<L: LazyBackend> {
    type Error: Error;

    type Dirty: MainToDeviceBufferDirty<L> + Send + Sync; // Allows for some cleaning to be done off-thread after a buffer has been used

    async fn copy_to_and_finish(
        self,
        dst: &L::DeviceOnlyBuffer,
        offset: usize,
    ) -> Result<Self::Dirty, Self::Error>;
}
#[async_trait]
pub trait MainToDeviceBufferMapped<L: LazyBackend> {
    type Error: Error;

    fn write_and_unmap(self, val: &[u8]) -> Result<L::MainToDeviceBufferUnmapped, Self::Error>;
}
#[async_trait]
pub trait MainToDeviceBufferDirty<L: LazyBackend> {
    type Error: Error;

    async fn clean(self) -> Result<L::MainToDeviceBufferMapped, Self::Error>;
}
#[async_trait]
pub trait DeviceOnlyBuffer<L: LazyBackend> {
    type CopyError: Error;

    fn backend(&self) -> &L;
    fn len(&self) -> usize;

    /// Fills (as much as possible) this buffer from the other buffer.
    /// If this buffer is smaller than the other then the other is truncated.
    /// If the other is smaller than this buffer then the data stored in the remaining space is
    /// implementation dependent, and should be treated as uninitialized.
    async fn copy_from(&mut self, other: &Self) -> Result<(), Self::CopyError>;
}

pub trait LazyBackend: Debug + Sized + Send + Sync + 'static {
    type BufferCreationError: Error;

    const CHUNK_SIZE: usize;

    type Utils: crate::compute_utils::Utils<Lazy<Self>>;
    type DeviceToMainBufferMapped: DeviceToMainBufferMapped<Self> + Sized + Send + Sync;
    type MainToDeviceBufferMapped: MainToDeviceBufferMapped<Self> + Sized + Send + Sync;
    type DeviceToMainBufferUnmapped: DeviceToMainBufferUnmapped<Self> + Sized + Send + Sync;
    type MainToDeviceBufferUnmapped: MainToDeviceBufferUnmapped<Self> + Sized + Send + Sync;
    type DeviceOnlyBuffer: DeviceOnlyBuffer<Self> + Sized + Send + Sync;

    fn get_utils(&self) -> &Self::Utils;
    fn create_device_only_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Result<Self::DeviceOnlyBuffer, Self::BufferCreationError>;

    fn create_device_to_main_memory(
        &self,
    ) -> Result<Self::DeviceToMainBufferUnmapped, Self::BufferCreationError>;
    fn create_main_to_device_memory(
        &self,
    ) -> Result<Self::MainToDeviceBufferMapped, Self::BufferCreationError>;
}

/// Wrap a lazy backend to keep some more state.
pub struct Lazy<L: LazyBackend> {
    pub lazy: Arc<L>,

    pub upload_buffers: Arc<WriteBufferRing<L>>,
    pub download_buffers: Arc<ReadBufferRing<L>>,
}

impl<L: LazyBackend> Clone for Lazy<L> {
    fn clone(&self) -> Self {
        Self {
            lazy: self.lazy.clone(),
            upload_buffers: self.upload_buffers.clone(),
            download_buffers: self.download_buffers.clone(),
        }
    }
}

impl<L: LazyBackend> Lazy<L> {
    pub async fn new_from(lazy: L, cfg: BufferRingConfig) -> Self {
        let backend = Arc::new(lazy);

        Self {
            upload_buffers: Arc::new(WriteBufferRing::new(backend.clone(), cfg).await),
            download_buffers: Arc::new(ReadBufferRing::new(backend.clone(), cfg).await),
            lazy: backend,
        }
    }
}

impl<L: LazyBackend> Debug for Lazy<L> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lazy (")?;
        self.lazy.fmt(f)?;
        write!(f, ")")
    }
}

// Map the lazy backend API on to the generic backend API
impl<L: LazyBackend> Backend for Lazy<L> {
    type BufferCreationError = L::BufferCreationError;
    type DeviceMemoryBlock = UnmappedLazyBuffer<L>;
    type MainMemoryBlock = MappedLazyBuffer<L>;
    type Utils = L::Utils;

    fn create_device_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Result<Self::DeviceMemoryBlock, Self::BufferCreationError> {
        UnmappedLazyBuffer::new(self.clone(), size, initial_data)
    }

    fn get_utils(&self) -> &Self::Utils {
        self.lazy.get_utils()
    }
}
