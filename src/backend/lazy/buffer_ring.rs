pub mod read;
pub mod write;

use crate::backend::lazy::LazyBackend;
use async_channel::{Receiver, Sender, TrySendError};
use async_trait::async_trait;
use perfect_derive::perfect_derive;
use std::error::Error;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

#[derive(Copy, Clone, Debug)]
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

#[perfect_derive(Debug)]
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

    /// Create a new buffer to be put in the pool
    async fn create_buffer(&self) -> Self::InitialBuffer;
    /// Perform whatever actions need to be done after a buffer has been used,
    /// before it is ready to be used again
    async fn clean(&self, buff: Self::FinalBuffer) -> Self::InitialBuffer;
}

impl<L: LazyBackend, Impl: BufferRingImpl<L> + 'static> BufferRing<L, Impl> {
    /// Adds a new buffer directly to our pool of buffers
    /// Panics on failure to send or if the future doesn't resolve immediately
    async fn add_buffer(&self) {
        let new_buffer = self.implementation.create_buffer().await;

        // Future should immediately resolve since we reserved space
        match self.buffer_return.try_send(new_buffer) {
            Ok(()) => return,
            Err(e) => match e {
                TrySendError::Full(_) => panic!("the pool was full - this is a bug"),
                TrySendError::Closed(_) => panic!("the pool was closed - this is probably a bug but probably also a hard one to fix"),
            },
        }
    }

    pub async fn new_from(implementation: Impl, config: BufferRingConfig) -> Self {
        let buffer_count = config.total_mem / L::CHUNK_SIZE;
        let (buffer_return, unused_buffers) = async_channel::bounded(buffer_count);

        let new_self = Self {
            config,
            unused_buffers,
            buffer_return,
            implementation: Arc::new(implementation),
        };

        for _ in 0..buffer_count {
            Self::add_buffer(&new_self).await
        }

        return new_self;
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

    /// Tries to do something with a buffer in this. If the thing fails, tries to recover by dumping
    /// the old buffer and creating a new one.
    ///
    /// Panics if the function errored and we were unable to create a new buffer for this pool
    async fn try_with_buffer_async<Fut, Res, Err>(
        &self,
        f: impl FnOnce(Impl::InitialBuffer) -> Fut,
    ) -> Result<Res, Err>
    where
        Fut: Future<Output = Result<(Res, Impl::FinalBuffer), Err>>,
        Err: Error,
    {
        let buffer = self.pop().await;

        // If something went wrong, dump the buffer and gen a new one to try to recover
        let res: Result<(Res, Impl::FinalBuffer), Err> = match f(buffer).await {
            Ok((res, dirty)) => {
                self.push(dirty);
                Ok(res)
            }
            Err(e) => {
                // Recover integrity of buffer pool
                self.add_buffer().await;
                Err(e)
            }
        };

        return res;
    }
}
