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
use std::fmt::Debug;
use std::sync::Arc;

pub mod buffer_ring;
pub mod memory;

// The new lazy API
#[async_trait]
pub trait DeviceToMainBufferUnmapped<L: LazyBackend> {
    async fn copy_from(&mut self, src: &L::DeviceOnlyBuffer, offset: usize);

    async fn map(self) -> L::DeviceToMainBufferMapped;
}
#[async_trait]
pub trait DeviceToMainBufferMapped<L: LazyBackend> {
    fn view(&self) -> &[u8; L::CHUNK_SIZE];

    async fn unmap(self) -> L::DeviceToMainBufferUnmapped;
}

#[async_trait]
pub trait MainToDeviceBufferUnmapped<L: LazyBackend> {
    async fn copy_to(&self, dst: &L::DeviceOnlyBuffer, offset: usize);

    async fn map(self) -> L::MainToDeviceBufferMapped;
}
#[async_trait]
pub trait MainToDeviceBufferMapped<L: LazyBackend> {
    fn view_mut(&mut self) -> &mut [u8; L::CHUNK_SIZE];

    async fn unmap(self) -> L::MainToDeviceBufferUnmapped;
}
#[async_trait]
pub trait DeviceOnlyBuffer<L: LazyBackend> {
    fn backend(&self) -> &L;
    fn len(&self) -> usize;

    /// Fills (as much as possible) this buffer from the other buffer.
    /// If this buffer is smaller than the other then the other is truncated.
    /// If the other is smaller than this buffer then the data stored in the remaining space is
    /// implementation dependent, and should be treated as uninitialized.
    async fn copy_from(&mut self, other: &Self);
}

pub trait LazyBackend: Debug + Sized {
    const CHUNK_SIZE: usize;

    type Utils: crate::compute_utils::Utils<Lazy<Self>>;
    type DeviceToMainBufferMapped: DeviceToMainBufferMapped<Self> + Sized;
    type MainToDeviceBufferMapped: MainToDeviceBufferMapped<Self> + Sized;
    type DeviceToMainBufferUnmapped: DeviceToMainBufferUnmapped<Self> + Sized;
    type MainToDeviceBufferUnmapped: MainToDeviceBufferUnmapped<Self> + Sized;
    type DeviceOnlyBuffer: DeviceOnlyBuffer<Self> + Sized;

    fn get_utils(&self) -> &Self::Utils;
    fn create_device_only_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Self::DeviceOnlyBuffer;

    fn create_device_to_main_memory(&self) -> Self::DeviceToMainBufferUnmapped;
    fn create_main_to_device_memory(&self) -> Self::DeviceToMainBufferUnmapped;
}

// Wrap a lazy backend to keep some more state
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Lazy<L: LazyBackend> {
    backend: Arc<L>,

    upload_buffers: Arc<WriteBufferRing<L>>,
    download_buffers: Arc<ReadBufferRing<L>>,
}

impl<L: LazyBackend> Lazy<L> {
    pub fn new_from(backend: L, cfg: BufferRingConfig) -> Self {
        let backend = Arc::new(backend);
        Self {
            upload_buffers: Arc::new(WriteBufferRing::new(backend.clone(), cfg)),
            download_buffers: Arc::new(ReadBufferRing::new(backend.clone(), cfg)),
            backend,
        }
    }
}

// Map the lazy backend API on to the generic backend API
impl<L: LazyBackend> Backend for Lazy<L> {
    type DeviceMemoryBlock = UnmappedLazyBuffer<L>;
    type MainMemoryBlock = MappedLazyBuffer<L>;
    type Utils = L::Utils;

    fn create_device_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Self::DeviceMemoryBlock {
        UnmappedLazyBuffer::new(
            self.backend.clone(),
            self.upload_buffers.clone(),
            self.download_buffers.clone(),
            size,
            initial_data,
        )
    }

    fn get_utils(&self) -> &Self::Utils {
        self.backend.get_utils()
    }
}
