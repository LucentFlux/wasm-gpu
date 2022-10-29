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
    AutoCommandBufferBuilder, BufferCopy, CommandBufferExecFuture, CommandBufferUsage,
    CopyBufferInfo, PrimaryAutoCommandBuffer,
};
use vulkano::sync::{FenceSignalFuture, GpuFuture};
use vulkano::{sync, DeviceSize};

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
        let mut builder = AutoCommandBufferBuilder::primary(
            self.backend.device.clone(),
            self.backend.queue_family_index,
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let source = other.buffer.clone();
        let destination = self.buffer.clone();

        let (source, destination) = match (source, destination) {
            (None, _) => return,
            (_, None) => return,
            (Some(source), Some(destination)) => (source, destination),
        };

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
    work: Option<Box<FenceSignalFuture<Box<dyn GpuFuture + Send>>>>,
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
) -> FenceSignalFuture<Box<dyn GpuFuture + Send>> {
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

    let prev_work = work.unwrap_or_else(|| Box::new(sync::now(backend.device.clone())));

    let future: Box<dyn GpuFuture + Send> = Box::new(
        prev_work
            .then_execute(backend.queue.clone(), command_buffer)
            .unwrap(),
    );
    let future = future.then_signal_fence_and_flush().unwrap();

    return future;
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

        let source = match source {
            None => return,
            Some(source) => source,
        };

        // Remap to wide pointer for dynamic dispatch - there may be a nicer way to get rust to do this :)
        let old_work = self.work.take().map(|old_work| {
            let old_work: Box<dyn GpuFuture + Send> = Box::new(*old_work);
            old_work
        });
        let new_work = add_copy_to_work(offset, source, destination, &self.backend, old_work);

        self.work = Some(Box::new(new_work));
    }

    async fn map(self) -> DeviceToMainBufferMapped {
        let Self {
            buffer,
            backend,
            work,
        } = self;

        // Complete all outstanding work
        if let Some(work) = work {
            // TODO: Switch to await once merged https://github.com/vulkano-rs/vulkano/pull/2051
            if !work.is_signaled().unwrap() {
                work.wait(None).unwrap();
            }
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

        let destination = match destination {
            None => return,
            Some(destination) => destination,
        };

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
