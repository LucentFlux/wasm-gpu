use crate::wgpu::async_device::AsyncDevice;
use std::ops::RangeBounds;
use wgpu::{Buffer, BufferAddress, BufferSlice, MapMode};

#[derive(Debug)]
pub struct AsyncBuffer
where
    Self: Send,
{
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
    ) -> BufferSlice {
        let slice = self.buffer.slice(bounds);

        self.device
            .do_async(|callback| slice.map_async(mode, callback))
            .await
            .expect("could not map buffer");

        return slice;
    }
}

impl AsRef<Buffer> for AsyncBuffer {
    fn as_ref(&self) -> &Buffer {
        &self.buffer
    }
}
