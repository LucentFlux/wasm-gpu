use crate::backend::lazy::buffer_ring::{BufferRing, BufferRingConfig, BufferRingImpl};
use crate::backend::lazy::{
    LazyBackend, MainToDeviceBufferDirty, MainToDeviceBufferMapped, MainToDeviceBufferUnmapped,
};
use async_trait::async_trait;
use std::sync::Arc;

pub type WriteBufferRing<L: LazyBackend> = BufferRing<L, WriteImpl<L>>;

pub struct WriteImpl<L: LazyBackend> {
    backend: Arc<L>,
}

#[async_trait]
impl<L: LazyBackend> BufferRingImpl<L> for WriteImpl<L> {
    type InitialBuffer = L::MainToDeviceBufferMapped;
    type FinalBuffer = <L::MainToDeviceBufferUnmapped as MainToDeviceBufferUnmapped<L>>::Dirty;

    async fn create_buffer(&self) -> Self::InitialBuffer {
        self.backend.create_main_to_device_memory()
    }

    async fn clean(&self, buff: Self::FinalBuffer) -> Self::InitialBuffer {
        buff.clean().await
    }
}

impl<L: LazyBackend> WriteBufferRing<L> {
    pub async fn new(backend: Arc<L>, config: BufferRingConfig) -> Self {
        BufferRing::new_from(WriteImpl { backend }, config).await
    }

    /// Copies a slice onto a GPU buffer
    pub async fn write_slice(&self, dst: &L::DeviceOnlyBuffer, offset: usize, slice: &[u8]) {
        assert_eq!(slice.len(), L::CHUNK_SIZE); // This should be checked at compile time but const generics are too buggy as of 23/10/2022

        let mut upload_buffer: L::MainToDeviceBufferMapped = self.pop().await;

        let upload_buffer = upload_buffer.write_and_unmap(slice).await;

        let upload_buffer = upload_buffer.copy_to_and_finish(dst, offset);

        self.push(upload_buffer);
    }
}
