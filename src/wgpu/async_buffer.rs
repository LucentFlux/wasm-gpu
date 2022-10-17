use crate::wgpu::async_device::AsyncDevice;
use anyhow::anyhow;
use std::ops::RangeBounds;
use wgpu::{Buffer, BufferAddress, BufferAsyncError, BufferSlice, MapMode};

pub struct AsyncBuffer {
    device: AsyncDevice,
    buffer: Buffer,
}

impl AsyncBuffer {
    pub fn new(device: AsyncDevice, buffer: Buffer) -> Self {
        Self { device, buffer }
    }

    pub async fn map_slice<S: RangeBounds<BufferAddress>>(
        &self,
        bounds: S,
        mode: MapMode,
    ) -> anyhow::Result<BufferSlice> {
        let slice = self.buffer.slice(bounds);

        let res = self
            .device
            .do_async(|callback| slice.map_async(mode, callback))
            .await;

        if let Err(e) = res {
            return Err(anyhow!("{}", e));
        }

        return Ok(slice);
    }
}

impl AsRef<Buffer> for AsyncBuffer {
    fn as_ref(&self) -> &Buffer {
        &self.buffer
    }
}
