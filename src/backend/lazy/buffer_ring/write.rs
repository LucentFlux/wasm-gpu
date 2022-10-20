use crate::backend::lazy::buffer_ring::{BufferRing, BufferRingConfig, BufferRingImpl};
use crate::backend::lazy::{
    DeviceToMainBufferUnmapped, LazyBackend, MainToDeviceBufferMapped, MainToDeviceBufferUnmapped,
};
use async_trait::async_trait;
use std::sync::Arc;

pub type WriteBufferRing<B: LazyBackend> = BufferRing<B::CHUNK_SIZE, WriteImpl<B>>;

struct WriteImpl<B: LazyBackend> {
    backend: Arc<B>,
}

#[async_trait]
impl<B: LazyBackend> BufferRingImpl<B::CHUNK_SIZE> for WriteImpl<B> {
    type InitialBuffer = B::MainToDeviceBufferMapped;
    type FinalBuffer = B::MainToDeviceBufferUnmapped;

    async fn create_buffer(&self) -> Self::InitialBuffer {
        self.backend.create_main_to_device_memory().map().await
    }

    async fn clean(&self, buff: Self::FinalBuffer) -> Self::InitialBuffer {
        buff.map().await
    }
}

impl<B: LazyBackend> WriteBufferRing<B> {
    pub fn new(backend: Arc<B>, config: BufferRingConfig) -> Self {
        WriteBufferRing::new(WriteImpl { backend }, config)
    }

    /// Copies a slice onto a GPU buffer
    pub async fn write_slice(
        &self,
        dst: &B::DeviceOnlyBuffer,
        offset: usize,
        slice: &[u8; B::CHUNK_SIZE],
    ) {
        let mut upload_buffer: B::MainToDeviceBufferMapped = self.pop().await;

        upload_buffer.view_mut().copy_from_slice(slice);

        let upload_buffer = upload_buffer.unmap().await;

        upload_buffer.copy_to(dst, offset).await;

        self.push(upload_buffer);

        return res;
    }
}
