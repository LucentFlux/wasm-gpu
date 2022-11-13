use crate::backend::lazy::buffer_ring::{
    BufferRing, BufferRingConfig, BufferRingImpl, NewBufferError,
};
use crate::backend::lazy::{
    DeviceToMainBufferDirty, DeviceToMainBufferMapped, DeviceToMainBufferUnmapped, LazyBackend,
};
use async_trait::async_trait;
use perfect_derive::perfect_derive;
use std::fmt::Debug;
use std::sync::Arc;
use thiserror::Error;

pub type ReadBufferRing<L: LazyBackend> = BufferRing<L, ReadImpl<L>>;

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
        self.backend.create_device_to_main_memory()
    }

    async fn clean(
        &self,
        buff: Self::FinalBuffer,
    ) -> Result<Self::InitialBuffer, Self::CleanError> {
        buff.clean().await
    }
}

#[derive(Error)]
#[perfect_derive(Debug)]
pub enum WithSlicePrimaryError<L: LazyBackend> {
    #[error("could not copy from or map buffer for reading")]
    CopyAndMapError(<L::DeviceToMainBufferUnmapped as DeviceToMainBufferUnmapped<L>>::Error),
    #[error("could not copy from or map buffer for reading")]
    ViewAndFinishError(<L::DeviceToMainBufferMapped as DeviceToMainBufferMapped<L>>::Error),
}

#[derive(Error)]
#[perfect_derive(Debug)]
pub enum WithSliceError<L: LazyBackend> {
    #[error("could not copy from or map buffer for reading")]
    SingleFailure(WithSlicePrimaryError<L>),
    #[error("could not add a new clean buffer when recovering from previous failure")]
    DoubleFailure(WithSlicePrimaryError<L>, NewBufferError<L, ReadImpl<L>>),
}

impl<L: LazyBackend> ReadBufferRing<L> {
    pub async fn new(
        backend: Arc<L>,
        config: BufferRingConfig,
    ) -> Result<BufferRing<L, ReadImpl<L>>, NewBufferError<L, ReadImpl<L>>> {
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
    ) -> Result<Res, WithSliceError<L>> {
        let mut download_buffer: L::DeviceToMainBufferUnmapped = self.pop().await;

        let download_buffer = download_buffer
            .copy_from_and_map(src, offset)
            .await
            .map_err(|e| WithSlicePrimaryError::CopyAndMapError(e));

        let res = download_buffer.and_then(|download_buffer| {
            download_buffer
                .view_and_finish(move |slice| {
                    assert_eq!(slice.len(), L::CHUNK_SIZE);
                    cont(slice)
                })
                .map_err(|e| WithSlicePrimaryError::ViewAndFinishError(e))
        });

        // If something went wrong, dump the buffer and gen a new one to try to recover
        let res = match res {
            Ok((res, dirty)) => {
                self.push(dirty);
                Ok(res)
            }
            Err(e) => {
                // Try to recover integrity of buffer pool
                let res2 = self.add_buffer().await;

                if let Err(e2) = res2 {
                    Err(WithSliceError::DoubleFailure(e, e2))
                } else {
                    Err(WithSliceError::SingleFailure(e))
                }
            }
        };

        return res;
    }
}
