use crate::backend::lazy::buffer_ring::{
    BufferRing, BufferRingConfig, BufferRingImpl, NewBufferError,
};
use crate::backend::lazy::{
    LazyBackend, MainToDeviceBufferDirty, MainToDeviceBufferMapped, MainToDeviceBufferUnmapped,
};
use async_trait::async_trait;
use perfect_derive::perfect_derive;
use std::sync::Arc;
use thiserror::Error;

pub type WriteBufferRing<L: LazyBackend> = BufferRing<L, WriteImpl<L>>;

#[perfect_derive(Debug)]
pub struct WriteImpl<L: LazyBackend> {
    backend: Arc<L>,
}

#[async_trait]
impl<L: LazyBackend> BufferRingImpl<L> for WriteImpl<L> {
    type InitialBuffer = L::MainToDeviceBufferMapped;
    type FinalBuffer = <L::MainToDeviceBufferUnmapped as MainToDeviceBufferUnmapped<L>>::Dirty;

    type NewError = L::BufferCreationError;
    type CleanError = <Self::FinalBuffer as MainToDeviceBufferDirty<L>>::Error;

    async fn create_buffer(&self) -> Result<Self::InitialBuffer, Self::NewError> {
        self.backend.try_create_main_to_device_memory()
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
pub enum WriteSliceError<L: LazyBackend> {
    #[error("could not write slice to buffer or unmap to device")]
    WriteAndUnmapError(<L::MainToDeviceBufferMapped as MainToDeviceBufferMapped<L>>::Error),
    #[error("could not copy data to device buffer or finish and mark buffer as dirty")]
    CopyToAndFinishError(<L::MainToDeviceBufferUnmapped as MainToDeviceBufferUnmapped<L>>::Error),
}

impl<L: LazyBackend> WriteBufferRing<L> {
    pub async fn try_new(
        backend: Arc<L>,
        config: BufferRingConfig,
    ) -> Result<BufferRing<L, WriteImpl<L>>, NewBufferError<L, WriteImpl<L>>> {
        BufferRing::new_from(WriteImpl { backend }, config).await
    }

    async fn write_slice_internal(
        &self,
        upload_buffer: L::MainToDeviceBufferMapped,
        dst: &L::DeviceOnlyBuffer,
        offset: usize,
        slice: &[u8],
    ) -> Result<((), <WriteImpl<L> as BufferRingImpl<L>>::FinalBuffer), WriteSliceError<L>> {
        let upload_buffer = upload_buffer
            .try_write_and_unmap(slice)
            .map_err(WriteSliceError::WriteAndUnmapError)?;

        let upload_buffer = upload_buffer
            .try_copy_to_and_finish(dst, offset)
            .await
            .map_err(WriteSliceError::CopyToAndFinishError)?;

        Ok(((), upload_buffer))
    }

    /// Copies a slice onto a GPU buffer
    ///
    /// Panics if the buffer errored and we were unable to create a new buffer for this pool
    pub async fn write_slice(
        &self,
        dst: &L::DeviceOnlyBuffer,
        offset: usize,
        slice: &[u8],
    ) -> Result<(), WriteSliceError<L>> {
        assert_eq!(slice.len(), L::CHUNK_SIZE); // This should be checked at compile time but const generics are too buggy as of 23/10/2022

        return self
            .try_with_buffer_async(|buffer| self.write_slice_internal(buffer, dst, offset, slice))
            .await;
    }
}
