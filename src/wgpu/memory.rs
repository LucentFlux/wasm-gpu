use crate::backend::lazy;
use crate::backend::lazy::{LazyBackend, MainToDeviceBufferDirty};
use crate::wgpu::async_buffer::AsyncBuffer;
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::WgpuBackendLazy;
use async_trait::async_trait;
use futures::FutureExt;
use ouroboros::self_referencing;
use std::cmp::min;
use std::fmt::Debug;
use wgpu::{BufferAddress, BufferSlice, MapMode};

async fn copy_max(
    backend: &WgpuBackendLazy,
    source: &wgpu::Buffer,
    source_offset: usize,
    destination: &wgpu::Buffer,
    destination_offset: usize,
) {
    let mut copy_command_encoder = backend
        .device
        .as_ref()
        .create_command_encoder(&Default::default());

    let err =
        "cannot handle more than 2^64 bytes of GPU RAM - this is probably a bug, unless you have more than 2^64 bytes of GPU RAM";
    let max_len = min(
        source.size() - u64::try_from(source_offset).expect(err),
        destination.size() - u64::try_from(destination_offset).expect(err),
    );

    copy_command_encoder.copy_buffer_to_buffer(
        source,
        source_offset as BufferAddress,
        destination,
        destination_offset as BufferAddress,
        max_len,
    );
    backend
        .queue
        .submit(vec![copy_command_encoder.finish()])
        .await;
}

fn new_buffer(
    device: &AsyncDevice,
    usage: wgpu::BufferUsages,
    size: usize,
    initial_data: Option<&[u8]>,
    map: bool,
) -> AsyncBuffer {
    let label = None;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label,
        size: size as BufferAddress,
        usage,
        mapped_at_creation: initial_data.is_some() | map,
    });

    if let Some(initial_data) = initial_data {
        buffer
            .as_ref()
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(initial_data);

        if !map {
            buffer.as_ref().unmap();
        }
    }

    buffer
}

#[derive(Debug)]
pub struct DeviceOnlyBuffer {
    backend: WgpuBackendLazy,
    buffer: AsyncBuffer,
}

impl DeviceOnlyBuffer {
    pub fn make_new(backend: WgpuBackendLazy, size: usize, initial_data: Option<&[u8]>) -> Self {
        let usage = wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::STORAGE;

        let buffer = new_buffer(&backend.device, usage, size, initial_data, false);

        Self { buffer, backend }
    }

    pub fn raw_buffer(&self) -> &AsyncBuffer {
        &self.buffer
    }
}

#[async_trait]
impl lazy::DeviceOnlyBuffer<WgpuBackendLazy> for DeviceOnlyBuffer {
    fn backend(&self) -> &WgpuBackendLazy {
        &self.backend
    }

    fn len(&self) -> usize {
        self.buffer.as_ref().size() as usize
    }

    async fn copy_from(&mut self, other: &Self) {
        copy_max(
            &self.backend,
            other.buffer.as_ref(),
            0,
            self.buffer.as_ref(),
            0,
        )
        .await
    }
}

#[derive(Debug)]
pub struct DeviceToMainBufferUnmapped {
    backend: WgpuBackendLazy,
    buffer: AsyncBuffer,
}

impl DeviceToMainBufferUnmapped {
    pub fn make_new(backend: WgpuBackendLazy) -> Self {
        let usage = wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ;
        let buffer = new_buffer(
            &backend.device,
            usage,
            WgpuBackendLazy::CHUNK_SIZE,
            None,
            false,
        );

        Self { backend, buffer }
    }
}

#[async_trait]
impl lazy::DeviceToMainBufferUnmapped<WgpuBackendLazy> for DeviceToMainBufferUnmapped {
    async fn copy_from_and_map(
        self,
        src: &DeviceOnlyBuffer,
        offset: usize,
    ) -> DeviceToMainBufferMapped {
        copy_max(
            &self.backend,
            src.buffer.as_ref(),
            offset,
            self.buffer.as_ref(),
            0,
        )
        .await;

        DeviceToMainBufferMappedAsyncSendBuilder {
            buffer: self.buffer,
            backend: self.backend,
            slice_builder: move |buffer| buffer.map_slice(.., MapMode::Read).boxed(),
        }
        .build()
        .await
    }
}

#[self_referencing]
#[derive(Debug)]
pub struct DeviceToMainBufferMapped {
    backend: WgpuBackendLazy,
    buffer: AsyncBuffer,
    #[borrows(buffer)]
    #[covariant]
    slice: BufferSlice<'this>,
}

#[async_trait]
impl lazy::DeviceToMainBufferMapped<WgpuBackendLazy> for DeviceToMainBufferMapped {
    type Dirty = DeviceToMainBufferToUnmap;

    fn view_and_finish<Res, F: FnOnce(&[u8]) -> Res>(self, callback: F) -> (Res, Self::Dirty) {
        let res = self.with_slice(move |slice: &BufferSlice| {
            let mapped = slice.get_mapped_range();
            let slice = mapped.as_ref();
            callback(slice)
        });

        return (res, DeviceToMainBufferToUnmap(self));
    }
}

#[derive(Debug)]
pub struct DeviceToMainBufferToUnmap(DeviceToMainBufferMapped);

#[async_trait]
impl lazy::DeviceToMainBufferDirty<WgpuBackendLazy> for DeviceToMainBufferToUnmap {
    async fn clean(self) -> DeviceToMainBufferUnmapped {
        let heads = self.0.into_heads();

        heads.buffer.as_ref().unmap();

        DeviceToMainBufferUnmapped {
            buffer: heads.buffer,
            backend: heads.backend,
        }
    }
}

#[self_referencing]
#[derive(Debug)]
pub struct MainToDeviceBufferMapped {
    backend: WgpuBackendLazy,
    buffer: AsyncBuffer,
    #[borrows(buffer)]
    #[covariant]
    slice: BufferSlice<'this>,
}

impl MainToDeviceBufferMapped {
    pub fn make_new(backend: WgpuBackendLazy) -> Self {
        let usage = wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE;
        let buffer = new_buffer(
            &backend.device,
            usage,
            WgpuBackendLazy::CHUNK_SIZE,
            None,
            true,
        );

        // We dig in to the internals a bit because we know we initialized the buffer as mappable
        MainToDeviceBufferMappedBuilder {
            buffer,
            backend,
            slice_builder: move |buffer: &AsyncBuffer| buffer.as_ref().slice(..),
        }
        .build()
    }
}

#[async_trait]
impl lazy::MainToDeviceBufferMapped<WgpuBackendLazy> for MainToDeviceBufferMapped {
    fn write_and_unmap(self, val: &[u8]) -> MainToDeviceBufferUnmapped {
        self.with_slice(|slice| slice.get_mapped_range_mut().copy_from_slice(val));

        let heads = self.into_heads();
        heads.buffer.as_ref().unmap();

        MainToDeviceBufferUnmapped {
            backend: heads.backend,
            buffer: heads.buffer,
        }
    }
}

#[derive(Debug)]
pub struct MainToDeviceBufferUnmapped {
    backend: WgpuBackendLazy,
    buffer: AsyncBuffer,
}

#[async_trait]
impl lazy::MainToDeviceBufferUnmapped<WgpuBackendLazy> for MainToDeviceBufferUnmapped {
    type Dirty = MainToDeviceBufferToMap;

    async fn copy_to_and_finish(self, dst: &DeviceOnlyBuffer, offset: usize) -> Self::Dirty {
        copy_max(
            &self.backend,
            self.buffer.as_ref(),
            0,
            dst.buffer.as_ref(),
            offset,
        )
        .await;

        MainToDeviceBufferToMap(self)
    }
}

#[derive(Debug)]
pub struct MainToDeviceBufferToMap(MainToDeviceBufferUnmapped);

#[async_trait]
impl MainToDeviceBufferDirty<WgpuBackendLazy> for MainToDeviceBufferToMap {
    async fn clean(self) -> MainToDeviceBufferMapped {
        MainToDeviceBufferMappedAsyncSendBuilder {
            buffer: self.0.buffer,
            backend: self.0.backend,
            slice_builder: move |buffer| buffer.map_slice(.., MapMode::Write).boxed(),
        }
        .build()
        .await
    }
}
