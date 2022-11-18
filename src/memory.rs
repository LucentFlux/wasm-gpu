//! Data can be either on the CPU or parallel computing device. Due to the existence of host functions,
//! we need to be able to map between these locations at runtime, with minimal overhead. This file deals with
//! these types

pub mod interleaved;

use crate::Backend;
use std::error::Error;
use std::fmt::Debug;

use crate::typed::ToRange;
use async_trait::async_trait;
use futures::TryFutureExt;
use perfect_derive::perfect_derive;
use thiserror::Error;

#[async_trait]
pub trait MemoryBlock<B>: Debug
where
    B: Backend,
{
    fn backend(&self) -> &B;

    fn len(&self) -> usize;
}

#[derive(Error)]
#[perfect_derive(Debug)]
pub enum MainMemoryResizeError<B: Backend<MainMemoryBlock = Mem>, Mem: MainMemoryBlock<B>> {
    /// Not unmapped, not resized
    #[error("memory could not be unmapped for resizing")]
    UnmapError(Mem::UnmapError, Mem),
    /// Not resized, left unmapped
    #[error("unmapped memory could not be resized")]
    DeviceResizeError(
        <<B as Backend>::DeviceMemoryBlock as DeviceMemoryBlock<B>>::ResizeError,
        B::DeviceMemoryBlock,
    ),
    /// Resized but not remapped
    #[error("memory could not be remapped once resized")]
    MapError(
        <<B as Backend>::DeviceMemoryBlock as DeviceMemoryBlock<B>>::MapError,
        B::DeviceMemoryBlock,
    ),
}

#[async_trait]
pub trait MainMemoryBlock<B>: MemoryBlock<B> + Sized + Send
where
    B: Backend<MainMemoryBlock = Self>,
{
    type UnmapError: Error + Send;
    type SliceError: Error + Send;
    type ResizeError: Error + Send = MainMemoryResizeError<B, Self>;

    async fn as_slice<S: ToRange<usize> + Send>(
        &self,
        bounds: S,
    ) -> Result<&[u8], Self::SliceError>;
    async fn as_slice_mut<S: ToRange<usize> + Send>(
        &mut self,
        bounds: S,
    ) -> Result<&mut [u8], Self::SliceError>;
    /// On failure to unmap, does nothing
    async fn unmap(self) -> Result<B::DeviceMemoryBlock, (Self::UnmapError, Self)>;

    /// Convenience method for writing blocks of data
    ///
    /// On error, no data was written.
    async fn write(&mut self, data: &[u8], offset: usize) -> Result<(), Self::SliceError> {
        let start = offset;
        let end = start + data.len();
        let slice = self.as_slice_mut(start..end).await?;
        slice.copy_from_slice(data);
        Ok(())
    }

    /// Resizes by moving off of main memory, reallocating and copying
    ///
    /// The `flush` portion of this operation is optional, and may be optimised away.
    async fn flush_resize(self, new_len: usize) -> Result<Self, Self::ResizeError> {
        let unmapped = self
            .unmap()
            .await
            .map_err(|(e, v)| MainMemoryResizeError::UnmapError(e, v))?;
        let unmapped = unmapped
            .resize(new_len)
            .await
            .map_err(|(e, v)| MainMemoryResizeError::DeviceResizeError(e, v))?;
        let remapped = unmapped
            .map()
            .await
            .map_err(|(e, v)| MainMemoryResizeError::MapError(e, v))?;

        Ok(remapped)
    }

    /// Convenience wrapper around `flush_resize` that adds more space
    async fn flush_extend(self, extra: usize) -> Result<Self, Self::ResizeError> {
        let len = self.len();
        self.flush_resize(len + extra).await
    }
}

#[derive(Error)]
#[perfect_derive(Debug)]
enum DeviceMemoryResizeError<B: Backend<DeviceMemoryBlock = Mem>, Mem: DeviceMemoryBlock<B>> {
    /// Couldn't create a new, larger buffer
    #[error("new memory block could not be allocated when resizing")]
    BufferCreationError(B::BufferCreationError),
    /// Could not copy to the new buffer. New buffer was deleted
    #[error("memory could not be copied into the new resized buffer")]
    CopyError(Mem::CopyError),
}

#[async_trait]
pub trait DeviceMemoryBlock<B>: MemoryBlock<B> + Sized
where
    B: Backend<DeviceMemoryBlock = Self>,
{
    type MapError: Error + Send;
    type CopyError: Error + Send;
    type ResizeError: Error + Send = DeviceMemoryResizeError<B, Self>;

    async fn map(self) -> Result<B::MainMemoryBlock, (Self::MapError, Self)>;
    async fn copy_from(&mut self, other: &B::DeviceMemoryBlock) -> Result<(), Self::CopyError>;

    /// Resizes by reallocation and copying
    async fn resize(self, new_len: usize) -> Result<Self, (Self::ResizeError, Self)> {
        let backend = self.backend();
        let mut new_buffer = backend
            .try_create_device_memory_block(new_len, None)
            .map_err(|e| (DeviceMemoryResizeError::BufferCreationError(e), self))?;
        if let Err(e) = new_buffer.copy_from(&self) {
            return Err((DeviceMemoryResizeError::CopyError(e), self));
        }

        Ok(new_buffer)
    }

    /// Convenience wrapper around `resize` that adds more space
    async fn extend(self, extra: usize) -> Result<Self, (Self::ResizeError, Self)> {
        let len = self.len();
        self.resize(len + extra).await
    }
}

pub fn limits_match<V: Ord>(n1: V, m1: Option<V>, n2: V, m2: Option<V>) -> bool {
    if n1 > n2 {
        return false;
    }
    return match (m1, m2) {
        (None, None) => true,
        (Some(m1), Some(m2)) => m1 >= m2,
        (_, _) => false,
    };
}
