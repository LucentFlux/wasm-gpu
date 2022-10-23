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
pub trait DeviceToMainBufferUnmapped<B: LazyBackend> {
    async fn copy_from(&mut self, src: &B::DeviceOnlyBuffer, offset: usize);

    async fn map(self) -> B::DeviceToMainBufferMapped;
}
#[async_trait]
pub trait DeviceToMainBufferMapped<B: LazyBackend> {
    fn view(&self) -> &[u8; B::CHUNK_SIZE];

    async fn unmap(self) -> B::DeviceToMainBufferUnmapped;
}

#[async_trait]
pub trait MainToDeviceBufferUnmapped<B: LazyBackend> {
    async fn copy_to(&self, dst: &B::DeviceOnlyBuffer, offset: usize);

    async fn map(self) -> B::MainToDeviceBufferMapped;
}
#[async_trait]
pub trait MainToDeviceBufferMapped<B: LazyBackend> {
    fn view_mut(&mut self) -> &mut [u8; B::CHUNK_SIZE];

    async fn unmap(self) -> B::MainToDeviceBufferUnmapped;
}
#[async_trait]
pub trait DeviceOnlyBuffer<B: LazyBackend> {
    fn backend(&self) -> &B;
    fn len(&self) -> usize;

    /// Fills (as much as possible) this buffer from the other buffer.
    /// If this buffer is smaller than the other then the other is truncated.
    /// If the other is smaller than this buffer then the data stored in the remaining space is
    /// implementation dependent, and should be treated as uninitialized.
    async fn copy_from(&mut self, other: &Self);
}

pub trait LazyBackend: Debug {
    const CHUNK_SIZE: usize;

    type Utils: crate::compute_utils::Utils<Self>;
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
pub struct Lazy<B: LazyBackend> {
    backend: Arc<B>,

    upload_buffers: Arc<WriteBufferRing<B>>,
    download_buffers: Arc<ReadBufferRing<B>>,
}

impl<B: LazyBackend> Lazy<B> {
    pub fn new_from(backend: B, cfg: BufferRingConfig) -> Self {
        let backend = Arc::new(backend);
        Self {
            upload_buffers: Arc::new(WriteBufferRing::new(backend.clone(), cfg)),
            download_buffers: Arc::new(ReadBufferRing::new(backend.clone(), cfg)),
            backend,
        }
    }
}

// Map the lazy backend API on to the generic backend API
impl<B: LazyBackend> Backend for Lazy<B> {
    type DeviceMemoryBlock = UnmappedLazyBuffer<Self>;
    type MainMemoryBlock = MappedLazyBuffer<Self>;
    type Utils = B::Utils;

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
