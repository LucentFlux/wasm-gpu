use crate::atomic_counter::AtomicCounter;
use crate::WgpuBackend;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;
use wgpu::{
    Buffer, BufferAddress, BufferAsyncError, BufferDescriptor, BufferUsages, Maintain,
    MaintainBase, MapMode,
};

// When the number of unused buffers is > total_buffers / DROP_RATIO, start discarding buffers
// as they are returned. Can also be interpreted as the maximum number of unused buffers at any point
// when no buffers are in use
const DROP_RATIO: usize = 4;

// The size of staging buffers to use
// TODO: Profile
pub const STAGING_BUFFER_SIZE: usize = 1 * 1024; // 1KB

pub struct BufferRing {
    device: Arc<wgpu::Device>,
    unused_buffers: Arc<Mutex<VecDeque<wgpu::Buffer>>>,
    total_buffers: AtomicUsize, // Used for discarding buffers that aren't being used fast enough
    map_mode: MapMode,
    usages: BufferUsages,
    mapped_at_creation: bool,

    // Used for debugging
    label: String,
    buffer_counter: AtomicCounter,
}

impl BufferRing {
    pub fn new(device: Arc<wgpu::Device>, label: String, map_mode: MapMode) -> Self {
        let usages = match map_mode {
            MapMode::Read => BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            MapMode::Write => BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC,
        };
        let mapped_at_creation = match map_mode {
            MapMode::Read => false,
            MapMode::Write => true,
        };
        Self {
            device,
            unused_buffers: Arc::new(Mutex::new(VecDeque::new())),
            total_buffers: AtomicUsize::new(0),
            map_mode,
            usages,
            mapped_at_creation,
            label,
            buffer_counter: AtomicCounter::new(),
        }
    }

    /// Gets a new buffer of size STAGING_BUFFER_SIZE. If map_mode is MapMode::Write, then the whole
    /// buffer is already mapped to CPU memory
    pub fn pop(&self) -> wgpu::Buffer {
        // On mapping we need to poll to trigger the callback
        self.device.poll(Maintain::Poll);

        // Assume faster to create than wait
        match self
            .unused_buffers
            .lock()
            .expect("unused buffer mutex was poisoned when popping")
            .pop_front()
        {
            None => {
                self.total_buffers.fetch_add(1, Ordering::AcqRel);
                let buffer_id = self.buffer_counter.next();
                self.device.create_buffer(&BufferDescriptor {
                    label: Some(format!("Staging buffer [{} #{}]", label, buffer_id).as_str()),
                    size: STAGING_BUFFER_SIZE as BufferAddress,
                    usage: self.usages,
                    mapped_at_creation: self.mapped_at_creation,
                })
            }
            Some(b) => b,
        }
    }

    fn destroy(&self, buffer: &wgpu::Buffer) {
        buffer.destroy();
        self.total_buffers.fetch_sub(1, Ordering::AcqRel);
    }

    /// Buffer *must* have come from this ring
    pub fn push(&self, buffer: wgpu::Buffer) {
        // Check to delete buffer
        let cutoff: usize = self.total_buffers.load(Ordering::Acquire) / DROP_RATIO;
        let len = self
            .unused_buffers
            .lock()
            .expect("unused buffer mutex was poisoned when checking len")
            .len();
        if len > cutoff {
            self.destroy(&buffer);
            return;
        }

        let unused_buffers = self.unused_buffers.clone();
        let after_buffer_processed = |res: Result<(), BufferAsyncError>| {
            if let Err(_) = res {
                self.destroy(&buffer);
                return;
            }

            unused_buffers
                .lock()
                .expect("unused buffer mutex was poisoned when pushing")
                .push_back(buffer);
        };

        match self.map_mode {
            MapMode::Read => {
                buffer.unmap();
                after_buffer_processed(Ok(()));
            }
            MapMode::Write => {
                buffer
                    .slice(..)
                    .map_async(MapMode::Write, after_buffer_processed);
            }
        };
    }
}
