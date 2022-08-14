use crate::Backend;
use wgpu::{Device, Queue};

pub struct WgpuBackend {
    pub device: Device,
    pub queue: Queue,
}

impl WgpuBackend {}

impl Backend for WgpuBackend {}
