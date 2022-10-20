use crate::backend::lazy::buffer_ring::{BufferRing, BufferRingConfig, BufferRingImpl};
use crate::backend::lazy::{DeviceToMainBufferMapped, DeviceToMainBufferUnmapped, LazyBackend};
use async_trait::async_trait;
use std::sync::Arc;

pub type ReadBufferRing<B: LazyBackend> = BufferRing<B::CHUNK_SIZE, ReadImpl<B>>;

struct ReadImpl<B: LazyBackend> {
    backend: Arc<B>,
}

#[async_trait]
impl<B: LazyBackend> BufferRingImpl<B::CHUNK_SIZE> for ReadImpl<B> {
    type InitialBuffer = B::DeviceToMainBufferUnmapped;
    type FinalBuffer = B::DeviceToMainBufferMapped;

    async fn create_buffer(&self) -> Self::InitialBuffer {
        self.backend.create_device_to_main_memory()
    }

    async fn clean(&self, buff: Self::FinalBuffer) -> Self::InitialBuffer {
        buff.unmap().await
    }
}

impl<B: LazyBackend> ReadBufferRing<B> {
    pub fn new(backend: Arc<B>, config: BufferRingConfig) -> Self {
        ReadBufferRing::new(ReadImpl { backend }, config)
    }

    /// Executes a closure with a slice of a GPU buffer.
    ///
    /// The slice generated has length BUFFER_SIZE
    pub async fn with_slice<Res, F: FnOnce(&[u8; BUFFER_SIZE]) -> Res>(
        &self,
        src: &B::DeviceOnlyBuffer,
        offset: usize,
        cont: F,
    ) -> Res {
        let mut download_buffer: B::DeviceToMainBufferUnmapped = self.pop().await;

        download_buffer.copy_from(src, offset).await;

        let download_buffer = download_buffer.map().await;

        let res = {
            let view = download_buffer.view();

            cont(view)
        };

        self.push(download_buffer);

        return res;
    }
}
