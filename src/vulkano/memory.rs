use crate::backend::lazy;
use crate::backend::lazy::LazyBackend;
use crate::vulkano::VulkanoBackendLazy;
use async_trait::async_trait;
use std::cmp::min;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::buffer::cpu_pool::CpuBufferPoolSubbuffer;
use vulkano::buffer::{
    BufferAccess, BufferContents, BufferUsage, CpuAccessibleBuffer, DeviceLocalBuffer,
    TypedBufferAccess,
};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, BufferCopy, CommandBufferExecFuture, CommandBufferUsage,
    CopyBufferInfo, PrimaryAutoCommandBuffer,
};
use vulkano::memory::pool::StandardMemoryPool;
use vulkano::sync::{FenceSignalFuture, GpuFuture, NowFuture};
use vulkano::{sync, DeviceSize};

fn copy<
    B1: BufferAccess + TypedBufferAccess + 'static,
    B2: BufferAccess + TypedBufferAccess + 'static,
>(
    src_offset: usize,
    dest_offset: usize,
    source: Arc<B1>,
    destination: Arc<B2>,
    backend: &VulkanoBackendLazy,
) -> FenceSignalFuture<CommandBufferExecFuture<NowFuture, PrimaryAutoCommandBuffer>> {
    let mut builder = AutoCommandBufferBuilder::primary(
        backend.device.clone(),
        backend.queue_family_index,
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    let src_offset = src_offset as DeviceSize;
    let dst_offset = dest_offset as DeviceSize;
    let size = min(
        source.len().saturating_sub(src_offset),
        destination.len().saturating_sub(dst_offset),
    );
    assert_ne!(
        size,
        0,
        "cannot copy nothing: source length: {}, dest length: {}, source offset: {}, dest offset: {}",
        source.len(),
        destination.len(),
        src_offset,
        dst_offset
    );
    let mut command = CopyBufferInfo::buffers(source, destination);
    command.regions[0] = BufferCopy {
        src_offset,
        dst_offset,
        size,
        ..Default::default()
    };

    builder.copy_buffer(command).unwrap();

    let command_buffer = builder.build().unwrap();

    let prev_work = sync::now(backend.device.clone());

    let future = prev_work
        .then_execute(backend.queue.clone(), command_buffer)
        .unwrap();
    let future = future.then_signal_fence_and_flush().unwrap();

    return future;
}

pub struct DeviceOnlyBuffer {
    backend: VulkanoBackendLazy,
    buffer: Option<Arc<DeviceLocalBuffer<[u8]>>>,
}

impl DeviceOnlyBuffer {
    pub fn new(backend: VulkanoBackendLazy, size: usize, initial_data: Option<&[u8]>) -> Self {
        if let Some(v) = initial_data {
            assert_eq!(
                v.len(),
                size,
                "initial data must match the length of the buffer"
            );
        }

        if size == 0 {
            return Self {
                buffer: None,
                backend,
            };
        }

        let usage = BufferUsage {
            transfer_src: true,
            transfer_dst: true,
            storage_buffer: true,
            ..Default::default()
        };
        let queue = vec![backend.queue_family_index];
        let buffer = match initial_data {
            None => {
                DeviceLocalBuffer::array(backend.device.clone(), size as DeviceSize, usage, queue)
                    .expect("unable to build device only initial buffer")
            }
            Some(data) => {
                let (buffer, future) = DeviceLocalBuffer::from_iter(
                    data.into_iter().map(|v| *v),
                    usage,
                    backend.queue.clone(),
                )
                .expect("unable to build device only populated buffer");

                let future = future
                    .then_signal_fence_and_flush()
                    .expect("failed to build data transfer command queue");
                future.wait(None).unwrap(); // TODO: Use async
                buffer
            }
        };
        Self {
            buffer: Some(buffer),
            backend,
        }
    }
}

#[async_trait]
impl lazy::DeviceOnlyBuffer<VulkanoBackendLazy> for DeviceOnlyBuffer {
    fn backend(&self) -> &VulkanoBackendLazy {
        &self.backend
    }

    fn len(&self) -> usize {
        match &self.buffer {
            None => 0,
            Some(buffer) => buffer.len() as usize,
        }
    }

    async fn copy_from(&mut self, other: &Self) {
        let source = other.buffer.clone();
        let destination = self.buffer.clone();

        let (source, destination) = match (source, destination) {
            (None, _) => return,
            (_, None) => return,
            (Some(source), Some(destination)) => (source, destination),
        };

        let future = copy(0, 0, source, destination, &self.backend);
        // TODO: Switch to await once merged
        future.wait(None).unwrap()
    }
}

pub struct DeviceToMainBufferUnmapped {
    backend: VulkanoBackendLazy,
    buffer: Arc<CpuAccessibleBuffer<[u8]>>,
}

impl DeviceToMainBufferUnmapped {
    pub fn new(backend: VulkanoBackendLazy) -> Self {
        let usage = BufferUsage {
            transfer_dst: true,
            ..Default::default()
        };
        let buffer = unsafe {
            CpuAccessibleBuffer::uninitialized_array(
                backend.device.clone(),
                VulkanoBackendLazy::CHUNK_SIZE as DeviceSize,
                usage,
                false,
            )
            .expect("failed to create a device-to-host buffer")
        };

        Self { backend, buffer }
    }
}

#[async_trait]
impl lazy::DeviceToMainBufferUnmapped<VulkanoBackendLazy> for DeviceToMainBufferUnmapped {
    async fn copy_from_and_map(
        self,
        src: &DeviceOnlyBuffer,
        offset: usize,
    ) -> DeviceToMainBufferMapped {
        let source = src.buffer.clone();
        let destination = self.buffer;

        if let Some(source) = source {
            let work = copy(offset, 0, source, destination.clone(), &self.backend);
            // TODO: Switch to await once merged https://github.com/vulkano-rs/vulkano/pull/2051
            work.wait(None).unwrap();
        };

        DeviceToMainBufferMapped {
            buffer: destination,
            backend: self.backend,
        }
    }
}

#[async_trait]
impl lazy::DeviceToMainBufferDirty<VulkanoBackendLazy> for DeviceToMainBufferUnmapped {
    async fn clean(self) -> DeviceToMainBufferUnmapped {
        self
    }
}

pub struct DeviceToMainBufferMapped {
    backend: VulkanoBackendLazy,
    buffer: Arc<CpuAccessibleBuffer<[u8]>>,
}

#[async_trait]
impl lazy::DeviceToMainBufferMapped<VulkanoBackendLazy> for DeviceToMainBufferMapped {
    type Dirty = DeviceToMainBufferUnmapped;

    fn view_and_finish<Res, F: FnOnce(&[u8]) -> Res>(self, callback: F) -> (Res, Self::Dirty) {
        let res = {
            let lock = self.buffer.read().unwrap();
            callback(lock.as_ref())
        };

        let unmapped = Self::Dirty {
            buffer: self.buffer,
            backend: self.backend,
        };

        return (res, unmapped);
    }
}

pub struct MainToDeviceBufferMapped {
    backend: VulkanoBackendLazy,
    buffer: Arc<CpuAccessibleBuffer<[u8]>>,
}

impl MainToDeviceBufferMapped {
    pub fn new(backend: VulkanoBackendLazy) -> Self {
        let usage = BufferUsage {
            transfer_src: true,
            ..Default::default()
        };
        let buffer = unsafe {
            CpuAccessibleBuffer::uninitialized_array(
                backend.device.clone(),
                VulkanoBackendLazy::CHUNK_SIZE as DeviceSize,
                usage,
                false,
            )
            .expect("failed to create a host-to-device buffer")
        };

        Self { backend, buffer }
    }
}

#[async_trait]
impl lazy::MainToDeviceBufferMapped<VulkanoBackendLazy> for MainToDeviceBufferMapped {
    async fn write_and_unmap(self, val: &[u8]) -> MainToDeviceBufferUnmapped {
        {
            let mut lock = self.buffer.write().unwrap();
            lock.copy_from_slice(val);
        }

        MainToDeviceBufferUnmapped {
            buffer: self.buffer,
            backend: self.backend,
        }
    }
}

#[async_trait]
impl lazy::MainToDeviceBufferDirty<VulkanoBackendLazy> for MainToDeviceBufferMapped {
    async fn clean(self) -> MainToDeviceBufferMapped {
        self
    }
}

pub struct MainToDeviceBufferUnmapped {
    backend: VulkanoBackendLazy,
    buffer: Arc<CpuAccessibleBuffer<[u8]>>,
}

#[async_trait]
impl lazy::MainToDeviceBufferUnmapped<VulkanoBackendLazy> for MainToDeviceBufferUnmapped {
    type Dirty = MainToDeviceBufferMapped;

    fn copy_to_and_finish(self, dst: &DeviceOnlyBuffer, offset: usize) -> MainToDeviceBufferMapped {
        let source = self.buffer.clone();
        let destination = dst.buffer.clone();

        if let Some(destination) = destination {
            let work = copy(0, offset, source, destination, &self.backend);

            // TODO: Switch to await once merged https://github.com/vulkano-rs/vulkano/pull/2051
            work.wait(None).unwrap();
        };

        MainToDeviceBufferMapped {
            buffer: self.buffer,
            backend: self.backend,
        }
    }
}
