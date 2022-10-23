use crate::backend::lazy::buffer_ring::{BufferRing, BufferRingConfig, BufferRingImpl};
use crate::backend::lazy::{
    DeviceToMainBufferUnmapped, LazyBackend, MainToDeviceBufferMapped, MainToDeviceBufferUnmapped,
};
use async_trait::async_trait;
use std::sync::Arc;

pub type WriteBufferRing<L: LazyBackend> = BufferRing<L, WriteImpl<L>>;

struct WriteImpl<L: LazyBackend> {
    backend: Arc<L>,
}

#[async_trait]
impl<L: LazyBackend> BufferRingImpl<L> for WriteImpl<L> {
    type InitialBuffer = L::MainToDeviceBufferMapped;
    type FinalBuffer = L::MainToDeviceBufferUnmapped;

    async fn create_buffer(&self) -> Self::InitialBuffer {
        self.backend.create_main_to_device_memory().map().await
    }

    async fn clean(&self, buff: Self::FinalBuffer) -> Self::InitialBuffer {
        buff.map().await
    }
}

impl<L: LazyBackend> WriteBufferRing<L> {
    pub fn new(backend: Arc<L>, config: BufferRingConfig) -> Self {
        BufferRing::new_from(WriteImpl { backend }, config)
    }

    /// Copies a slice onto a GPU buffer
    pub async fn write_slice(
        &self,
        dst: &L::DeviceOnlyBuffer,
        offset: usize,
        slice: &[u8; L::CHUNK_SIZE],
    ) {
        let mut upload_buffer: L::MainToDeviceBufferMapped = self.pop().await;

        upload_buffer.view_mut().copy_from_slice(slice);

        let upload_buffer = upload_buffer.unmap().await;

        upload_buffer.copy_to(dst, offset).await;

        self.push(upload_buffer);
    }
}
