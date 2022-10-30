use crate::backend::lazy::buffer_ring::{BufferRing, BufferRingConfig, BufferRingImpl};
use crate::backend::lazy::{
    DeviceToMainBufferDirty, DeviceToMainBufferMapped, DeviceToMainBufferUnmapped, LazyBackend,
};
use async_trait::async_trait;
use std::sync::Arc;

pub type ReadBufferRing<L: LazyBackend> = BufferRing<L, ReadImpl<L>>;

pub struct ReadImpl<B: LazyBackend> {
    backend: Arc<B>,
}

#[async_trait]
impl<L: LazyBackend> BufferRingImpl<L> for ReadImpl<L> {
    type InitialBuffer = L::DeviceToMainBufferUnmapped;
    type FinalBuffer = <L::DeviceToMainBufferMapped as DeviceToMainBufferMapped<L>>::Dirty;

    async fn create_buffer(&self) -> Self::InitialBuffer {
        self.backend.create_device_to_main_memory()
    }

    async fn clean(&self, buff: Self::FinalBuffer) -> Self::InitialBuffer {
        buff.clean().await
    }
}

impl<L: LazyBackend> ReadBufferRing<L> {
    pub async fn new(backend: Arc<L>, config: BufferRingConfig) -> Self {
        BufferRing::new_from(ReadImpl { backend }, config).await
    }

    /// Executes a closure with a slice of a GPU buffer.
    ///
    /// The slice generated has length BUFFER_SIZE
    pub async fn with_slice<Res, F: FnOnce(&[u8]) -> Res>(
        &self,
        src: &L::DeviceOnlyBuffer,
        offset: usize,
        cont: F,
    ) -> Res {
        let mut download_buffer: L::DeviceToMainBufferUnmapped = self.pop().await;

        let download_buffer = download_buffer.copy_from_and_map(src, offset).await;

        let (res, download_buffer) = download_buffer.view_and_finish(move |slice| {
            assert_eq!(slice.len(), L::CHUNK_SIZE);
            cont(slice)
        });

        self.push(download_buffer);

        return res;
    }
}
