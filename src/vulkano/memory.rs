use crate::backend::lazy;
use crate::backend::lazy::LazyBackend;
use crate::vulkano::VulkanoBackendLazy;
use async_trait::async_trait;
use std::cmp::min;
use std::ops::Deref;
use std::sync::Arc;
use vulkano::buffer::{
    BufferAccess, BufferUsage, CpuAccessibleBuffer, DeviceLocalBuffer, TypedBufferAccess,
};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, BufferCopy, CommandBufferUsage, CopyBufferInfo,
};
use vulkano::sync::GpuFuture;
use vulkano::{sync, DeviceSize};

pub struct DeviceOnlyBuffer {
    backend: VulkanoBackendLazy,
    buffer: Arc<DeviceLocalBuffer<[u8]>>,
}

impl DeviceOnlyBuffer {
    pub fn new(backend: VulkanoBackendLazy, size: usize, initial_data: Option<&[u8]>) -> Self {
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
        Self { buffer, backend }
    }
}

#[async_trait]
impl lazy::DeviceOnlyBuffer<VulkanoBackendLazy> for DeviceOnlyBuffer {
    fn backend(&self) -> &VulkanoBackendLazy {
        &self.backend
    }

    fn len(&self) -> usize {
        self.buffer.len() as usize
    }

    async fn copy_from(&mut self, other: &Self) {
        let mut builder = AutoCommandBufferBuilder::primary(
            self.backend.device.clone(),
            self.backend.queue_family_index,
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let source = other.buffer.clone();
        let destination = self.buffer.clone();

        let mut command = CopyBufferInfo::buffers(source, destination);

        builder.copy_buffer(command).unwrap();

        let command_buffer = builder.build().unwrap();

        let future = sync::now(self.backend.device.clone())
            .then_execute(self.backend.queue.clone(), command_buffer)
            .unwrap()
            .then_signal_fence_and_flush()
            .unwrap();

        // TODO: Switch to await once merged
        future.wait(None).unwrap()
    }
}

pub struct DeviceToMainBufferMapped {
    backend: VulkanoBackendLazy,
    buffer: Arc<CpuAccessibleBuffer<[u8]>>,
}

#[async_trait]
impl lazy::DeviceToMainBufferMapped<VulkanoBackendLazy> for DeviceToMainBufferMapped {
    fn view<Res, F: FnOnce(&[u8]) -> Res>(&self, callback: F) -> Res {
        let lock = self.buffer.read().unwrap();
        callback(lock.deref())
    }

    async fn unmap(self) -> DeviceToMainBufferUnmapped {
        // No locks at this point, so can just move
        DeviceToMainBufferUnmapped {
            buffer: self.buffer,
            backend: self.backend,
            work: None,
        }
    }
}

pub struct DeviceToMainBufferUnmapped {
    backend: VulkanoBackendLazy,
    buffer: Arc<CpuAccessibleBuffer<[u8]>>,
    work: Option<Box<dyn GpuFuture + Send>>,
}

fn add_copy_to_work<
    B1: BufferAccess + TypedBufferAccess + 'static,
    B2: BufferAccess + TypedBufferAccess + 'static,
>(
    offset: usize,
    source: Arc<B1>,
    destination: Arc<B2>,
    backend: &VulkanoBackendLazy,
    mut work: Option<Box<dyn GpuFuture + Send>>,
) -> Box<dyn GpuFuture + Send> {
    let mut builder = AutoCommandBufferBuilder::primary(
        backend.device.clone(),
        backend.queue_family_index,
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    let offset = offset as DeviceSize;
    let size = min(source.len() - offset, destination.len());
    let mut command = CopyBufferInfo::buffers(source, destination);
    command.regions[0] = BufferCopy {
        src_offset: offset,
        dst_offset: 0,
        size,
        ..Default::default()
    };

    builder.copy_buffer(command).unwrap();

    let command_buffer = builder.build().unwrap();

    let prev_work = work
        .take()
        .unwrap_or_else(|| Box::new(sync::now(backend.device.clone())));

    let future = prev_work
        .then_execute(backend.queue.clone(), command_buffer)
        .unwrap()
        .then_signal_semaphore_and_flush()
        .unwrap();

    return Box::new(future);
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

        Self {
            backend,
            buffer,
            work: None,
        }
    }
}

#[async_trait]
impl lazy::DeviceToMainBufferUnmapped<VulkanoBackendLazy> for DeviceToMainBufferUnmapped {
    async fn copy_from(&mut self, src: &DeviceOnlyBuffer, offset: usize) {
        let source = src.buffer.clone();
        let destination = self.buffer.clone();

        let new_work =
            add_copy_to_work(offset, source, destination, &self.backend, self.work.take());

        self.work = Some(new_work);
    }

    async fn map(self) -> DeviceToMainBufferMapped {
        let Self {
            buffer,
            backend,
            work,
        } = self;

        // Complete all outstanding work
        if let Some(work) = work {
            let fence = work.then_signal_fence_and_flush().unwrap();
            // TODO: Switch to await once merged https://github.com/vulkano-rs/vulkano/pull/2051
            fence.wait(None).unwrap()
        }

        DeviceToMainBufferMapped { buffer, backend }
    }
}

pub struct MainToDeviceBufferMapped {
    backend: VulkanoBackendLazy,
    buffer: Arc<CpuAccessibleBuffer<[u8]>>,
}

#[async_trait]
impl lazy::MainToDeviceBufferMapped<VulkanoBackendLazy> for MainToDeviceBufferMapped {
    fn write(&mut self, val: &[u8]) {
        let mut lock = self.buffer.write().unwrap();
        lock.copy_from_slice(val)
    }

    async fn unmap(self) -> MainToDeviceBufferUnmapped {
        // No locks at this point, so can just move
        MainToDeviceBufferUnmapped {
            buffer: self.buffer,
            backend: self.backend,
        }
    }
}

pub struct MainToDeviceBufferUnmapped {
    backend: VulkanoBackendLazy,
    buffer: Arc<CpuAccessibleBuffer<[u8]>>,
}

impl MainToDeviceBufferUnmapped {
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
impl lazy::MainToDeviceBufferUnmapped<VulkanoBackendLazy> for MainToDeviceBufferUnmapped {
    async fn copy_to(&self, dst: &DeviceOnlyBuffer, offset: usize) {
        let source = self.buffer.clone();
        let destination = dst.buffer.clone();

        let new_work = add_copy_to_work(offset, source, destination, &self.backend, None);

        let fence = new_work.then_signal_fence_and_flush().unwrap();
        // TODO: Switch to await once merged https://github.com/vulkano-rs/vulkano/pull/2051
        fence.wait(None).unwrap()
    }

    async fn map(self) -> MainToDeviceBufferMapped {
        let Self { buffer, backend } = self;

        MainToDeviceBufferMapped { buffer, backend }
    }
}
