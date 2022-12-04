use crate::wgpu::async_device::AsyncDevice;
use std::sync::Arc;
use wgpu::{CommandBuffer, Queue};

#[derive(Clone, Debug)]
pub struct AsyncQueue {
    device: AsyncDevice,
    queue: Arc<Queue>,
}

impl AsyncQueue {
    pub fn new(device: AsyncDevice, queue: Queue) -> Self {
        Self {
            device,
            queue: Arc::new(queue),
        }
    }

    pub async fn submit<I: IntoIterator<Item = CommandBuffer> + Send>(&self, command_buffers: I) {
        let queue_ref = self.queue.clone();
        self.device
            .do_async(move |callback| {
                queue_ref.submit(command_buffers);
                queue_ref.on_submitted_work_done(|| callback(()));
            })
            .await
    }
}
