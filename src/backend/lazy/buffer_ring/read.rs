use crate::backend::lazy::buffer_ring::{
    BufferRing, BufferRingConfig, BufferRingImpl, NewBufferError,
};
use crate::backend::lazy::{
    DeviceToMainBufferDirty, DeviceToMainBufferMapped, DeviceToMainBufferUnmapped, LazyBackend,
};
use async_trait::async_trait;
use perfect_derive::perfect_derive;
use std::sync::Arc;
use thiserror::Error;

pub type ReadBufferRing<L: LazyBackend> = BufferRing<L, ReadImpl<L>>;

#[perfect_derive(Debug)]
pub struct ReadImpl<B: LazyBackend> {
    backend: Arc<B>,
}

#[async_trait]
impl<L: LazyBackend> BufferRingImpl<L> for ReadImpl<L> {
    type InitialBuffer = L::DeviceToMainBufferUnmapped;
    type FinalBuffer = <L::DeviceToMainBufferMapped as DeviceToMainBufferMapped<L>>::Dirty;
    type NewError = L::BufferCreationError;
    type CleanError = <Self::FinalBuffer as DeviceToMainBufferDirty<L>>::Error;

    async fn create_buffer(&self) -> Result<Self::InitialBuffer, Self::NewError> {
        self.backend.try_create_device_to_main_memory()
    }

    async fn clean(
        &self,
        buff: Self::FinalBuffer,
    ) -> Result<Self::InitialBuffer, Self::CleanError> {
        buff.try_clean().await
    }
}

#[derive(Error)]
#[perfect_derive(Debug)]
pub enum WithSliceError<L: LazyBackend> {
    #[error("could not copy from or map buffer for reading")]
    CopyAndMapError(<L::DeviceToMainBufferUnmapped as DeviceToMainBufferUnmapped<L>>::Error),
    #[error("could not view the buffer as a slice on the host or finish and mark buffer as dirty")]
    ViewAndFinishError(<L::DeviceToMainBufferMapped as DeviceToMainBufferMapped<L>>::Error),
}

impl<L: LazyBackend> ReadBufferRing<L> {
    pub async fn try_new(
        backend: Arc<L>,
        config: BufferRingConfig,
    ) -> Result<BufferRing<L, ReadImpl<L>>, NewBufferError<L, ReadImpl<L>>> {
        BufferRing::new_from(ReadImpl { backend }, config).await
    }

    async fn with_slice_internal<Res, F: FnOnce(&[u8]) -> Res>(
        &self,
        download_buffer: L::DeviceToMainBufferUnmapped,
        src: &L::DeviceOnlyBuffer,
        offset: usize,
        cont: F,
    ) -> Result<(Res, <ReadImpl<L> as BufferRingImpl<L>>::FinalBuffer), WithSliceError<L>> {
        let download_buffer = download_buffer
            .try_copy_from_and_map(src, offset)
            .await
            .map_err(WithSliceError::CopyAndMapError)?;

        let (res, dirty) = download_buffer
            .try_view_and_finish(move |slice| {
                assert_eq!(slice.len(), L::CHUNK_SIZE);
                cont(slice)
            })
            .map_err(WithSliceError::ViewAndFinishError)?;

        return Ok((res, dirty));
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
    ) -> Result<Res, WithSliceError<L>> {
        return self
            .try_with_buffer_async(|buffer| self.with_slice_internal(buffer, src, offset, cont))
            .await;
    }
}
