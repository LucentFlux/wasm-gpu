pub mod read;
pub mod write;

use crate::backend::lazy::LazyBackend;
use async_channel::{Receiver, Sender};
use async_trait::async_trait;
use std::sync::Arc;

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
trait BufferRingImpl<L: LazyBackend>: Send + Sync {
    type InitialBuffer: Send;
    type FinalBuffer: Send;

    /// Create a new buffer to be put in the pool
    async fn create_buffer(&self) -> Self::InitialBuffer;
    /// Perform whatever actions need to be done after a buffer has been used,
    /// before it is ready to be used again
    async fn clean(&self, buff: Self::FinalBuffer) -> Self::InitialBuffer;
}

impl<L: LazyBackend, Impl: BufferRingImpl<L> + 'static> BufferRing<L, Impl> {
    pub async fn new_from(implementation: Impl, config: BufferRingConfig) -> Self {
        let buffer_count = config.total_mem / L::CHUNK_SIZE;
        let (buffer_return, unused_buffers) = async_channel::bounded(buffer_count);
        for _ in 0..buffer_count {
            let new_buffer = implementation.create_buffer().await;

            // Future should immediately resolve since we reserved space
            let fut = buffer_return.send(new_buffer);
            fut.await
                .expect("failed to initialize buffers for data transfer")
        }

        Self {
            config,
            unused_buffers,
            buffer_return,
            implementation: Arc::new(implementation),
        }
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
