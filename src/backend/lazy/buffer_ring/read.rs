use crate::backend::lazy::buffer_ring::{BufferRing, BufferRingConfig, BufferRingImpl};
use crate::backend::lazy::{
    DeviceToMainBufferDirty, DeviceToMainBufferMapped, DeviceToMainBufferUnmapped, LazyBackend,
};
use async_trait::async_trait;
use perfect_derive::perfect_derive;
use std::sync::Arc;

pub type ReadBufferRing<L: LazyBackend> = BufferRing<L, ReadImpl<L>>;

#[perfect_derive(Debug)]
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
    pub async fn new(backend: Arc<L>, config: BufferRingConfig) -> BufferRing<L, ReadImpl<L>> {
        BufferRing::new_from(ReadImpl { backend }, config).await
    }

    /// Executes a closure with a slice of a GPU buffer.
    ///
    /// The slice generated has length BUFFER_SIZE
    ///
    /// Panics if the buffer errored and we were unable to create a new buffer for this pool
    pub async fn with_slice<Res, F: FnOnce(&[u8]) -> Res>(
        &self,
        src: &L::DeviceOnlyBuffer,
        offset: usize,
        cont: F,
    ) -> Res {
        let download_buffer = self.pop().await;

        let download_buffer = download_buffer.copy_from_and_map(src, offset).await;

        let (res, dirty) = download_buffer.view_and_finish(move |slice| {
            assert_eq!(slice.len(), L::CHUNK_SIZE);
            cont(slice)
        });

        self.push(dirty);

        res
    }
}
