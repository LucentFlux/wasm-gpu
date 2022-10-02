use crate::wgpu::async_device::AsyncDevice;
use std::ops::RangeBounds;
use std::sync::Arc;
use wgpu::{Buffer, BufferAddress, BufferSlice, MapMode};

pub struct AsyncBuffer {
    device: Arc<AsyncDevice>,
    buffer: Buffer,
}

impl AsyncBuffer {
    pub fn new(device: Arc<AsyncDevice>, buffer: Buffer) -> Self {
        Self { device, buffer }
    }

    pub async fn map_slice<S: RangeBounds<BufferAddress>>(
        &self,
        bounds: S,
        mode: MapMode,
    ) -> anyhow::Result<BufferSlice> {
        let slice = self.buffer.slice(bounds);

        self.device
            .do_async(|callback| slice.map_async(mode, callback))
            .await?;

        return Ok(slice);
    }
}

impl AsRef<Buffer> for AsyncBuffer {
    fn as_ref(&self) -> &Buffer {
        &self.buffer
    }
}
