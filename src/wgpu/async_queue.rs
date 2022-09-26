use crate::wgpu::async_device::AsyncDevice;
use std::sync::Arc;
use wgpu::{CommandBuffer, Queue};

#[derive(Clone, Debug)]
pub struct AsyncQueue {
    device: Arc<AsyncDevice>,
    queue: Queue,
}

impl AsyncQueue {
    pub fn new(device: Arc<AsyncDevice>, queue: Queue) -> Self {
        Self { device, queue }
    }

    pub async fn submit<I: IntoIterator<Item = CommandBuffer>>(&self, command_buffers: I) {
        self.device
            .do_async(move |callback| {
                self.queue.submit(command_buffers);
                self.queue.on_submitted_work_done(callback);
            })
            .await
    }
}
