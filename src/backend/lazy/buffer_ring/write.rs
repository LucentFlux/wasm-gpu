use crate::backend::lazy::buffer_ring::{BufferRing, BufferRingConfig, BufferRingImpl};
use crate::backend::lazy::{
    LazyBackend, MainToDeviceBufferDirty, MainToDeviceBufferMapped, MainToDeviceBufferUnmapped,
};
use async_trait::async_trait;
use perfect_derive::perfect_derive;
use std::sync::Arc;

pub type WriteBufferRing<L: LazyBackend> = BufferRing<L, WriteImpl<L>>;

#[perfect_derive(Debug)]
pub struct WriteImpl<L: LazyBackend> {
    backend: Arc<L>,
}

#[async_trait]
impl<L: LazyBackend> BufferRingImpl<L> for WriteImpl<L> {
    type InitialBuffer = L::MainToDeviceBufferMapped;
    type FinalBuffer = <L::MainToDeviceBufferUnmapped as MainToDeviceBufferUnmapped<L>>::Dirty;

    async fn create_buffer(&self) -> Self::InitialBuffer {
        self.backend.try_create_main_to_device_memory()
    }

    async fn clean(&self, buff: Self::FinalBuffer) -> Self::InitialBuffer {
        buff.clean().await
    }
}

impl<L: LazyBackend> WriteBufferRing<L> {
    pub async fn new(backend: Arc<L>, config: BufferRingConfig) -> BufferRing<L, WriteImpl<L>> {
        BufferRing::new_from(WriteImpl { backend }, config).await
    }

    /// Copies a slice onto a GPU buffer
    ///
    /// Panics if the buffer errored and we were unable to create a new buffer for this pool
    pub async fn write_slice(&self, dst: &L::DeviceOnlyBuffer, offset: usize, slice: &[u8]) {
        assert_eq!(slice.len(), L::CHUNK_SIZE); // This should be checked at compile time but const generics are too buggy as of 23/10/2022

        let upload_buffer = self.pop().await;

        let upload_buffer = upload_buffer.write_and_unmap(slice);

        let upload_buffer = upload_buffer.copy_to_and_finish(dst, offset).await;

        self.push(upload_buffer);
    }
}
