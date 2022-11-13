pub mod read;
pub mod write;

use crate::backend::lazy::LazyBackend;
use async_channel::{Receiver, Sender, TrySendError};
use async_trait::async_trait;
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;
use thiserror::Error;

#[derive(Copy, Clone)]
pub struct BufferRingConfig {
    /// A ring will allocate this amount of memory for moving data
    pub total_mem: usize,
}

impl Default for BufferRingConfig {
    fn default() -> Self {
        Self {
            total_mem: 16 * 1024 * 1024, // 16MB
        }
    }
}

pub struct BufferRing<L: LazyBackend, Impl: BufferRingImpl<L>> {
    config: BufferRingConfig,

    unused_buffers: Receiver<Impl::InitialBuffer>,
    buffer_return: Sender<Impl::InitialBuffer>,

    implementation: Arc<Impl>,
}

/// Pulls out the shared logic between send and receive
#[async_trait]
pub trait BufferRingImpl<L: LazyBackend>: Send + Sync {
    type InitialBuffer: Send;
    type FinalBuffer: Send;
    type NewError: Error;
    type CleanError: Error;

    /// Create a new buffer to be put in the pool
    async fn create_buffer(&self) -> Result<Self::InitialBuffer, Self::NewError>;
    /// Perform whatever actions need to be done after a buffer has been used,
    /// before it is ready to be used again
    async fn clean(&self, buff: Self::FinalBuffer)
        -> Result<Self::InitialBuffer, Self::CleanError>;
}

#[derive(Error)]
pub enum NewBufferError<L: LazyBackend, Impl: BufferRingImpl<L> + 'static> {
    #[error("new buffer could not be allocated to be added to the pool")]
    CreationError(Impl::NewError),
    #[error("the pool was closed - this implies a panic somewhere else")]
    PoolClosed,
}

impl<L: LazyBackend, Impl: BufferRingImpl<L> + 'static> Debug for NewBufferError<L, Impl> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NewBufferError::CreationError(e) => write!(f, "buffer creation error: {:?}", e),
            NewBufferError::PoolClosed => write!(f, "pool closed"),
        }
    }
}

impl<L: LazyBackend, Impl: BufferRingImpl<L> + 'static> BufferRing<L, Impl> {
    /// Adds a new buffer directly to our pool of buffers
    /// Panics on failure to send or if the future doesn't resolve immediately
    async fn add_buffer(&self) -> Result<(), NewBufferError<L, Impl>> {
        let new_buffer = self
            .implementation
            .create_buffer()
            .await
            .map_err(|e| NewBufferError::CreationError(e))?;

        // Future should immediately resolve since we reserved space
        match self.buffer_return.try_send(new_buffer) {
            Ok(()) => return Ok(()),
            Err(e) => match e {
                TrySendError::Full(_) => panic!("the pool was full - this is a bug"),
                TrySendError::Closed(_) => return Err(NewBufferError::PoolClosed),
            },
        }
    }

    pub async fn new_from(
        implementation: Impl,
        config: BufferRingConfig,
    ) -> Result<Self, NewBufferError<L, Impl>> {
        let buffer_count = config.total_mem / L::CHUNK_SIZE;
        let (buffer_return, unused_buffers) = async_channel::bounded(buffer_count);

        let new_self = Self {
            config,
            unused_buffers,
            buffer_return,
            implementation: Arc::new(implementation),
        };

        for _ in 0..buffer_count {
            Self::add_buffer(&new_self).await?
        }

        return Ok(new_self);
    }

    /// Gets a new buffer of size STAGING_BUFFER_SIZE. If map_mode is MapMode::Write, then the whole
    /// buffer is already mapped to CPU memory
    async fn pop(&self) -> Impl::InitialBuffer {
        return self
            .unused_buffers
            .recv()
            .await
            .expect("buffer ring stream closed on receiving");
    }

    /// Buffer *must* have come from this ring. Executes in a tokio task
    fn push(&self, buffer: Impl::FinalBuffer) {
        let ret = self.buffer_return.clone();
        let local_impl = self.implementation.clone();
        tokio::task::spawn(async move {
            let buffer = local_impl.clean(buffer).await;

            ret.send(buffer)
                .await
                .expect("buffer ring stream closed on sending");
        });
    }
}
