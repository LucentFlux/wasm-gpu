mod async_buffer;
mod async_device;
mod async_queue;
mod compute_utils;
mod memory;

use crate::backend::lazy::{Lazy, LazyBackend, NewBuffersError};
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::async_queue::AsyncQueue;
use crate::wgpu::compute_utils::WgpuComputeUtils;
use crate::BufferRingConfig;
use itertools::Itertools;
use std::fmt::Debug;
use thiserror::Error;

const CHUNK_SIZE: usize = 1024;

/// Things that we *can* use if supported, but don't need
pub struct WgpuUsefulFeatures {
    pub supports_f64: bool,
}

impl WgpuUsefulFeatures {
    fn intersect(self, adapter: &wgpu::Adapter) -> Self {
        let new_features = adapter.features().intersect(self.as_features());

        let supports_f64 = new_features.contains(wgpu::Features::SHADER_FLOAT64);

        Self { supports_f64 }
    }

    fn as_features(&self) -> wgpu::Features {
        let mut res = wgpu::Features::empty();

        // Enumerate self
        res.set(wgpu::Features::SHADER_FLOAT64, true);

        res
    }
}

/// Default to be used in config, where true means "try to use"
impl Default for WgpuUsefulFeatures {
    fn default() -> Self {
        Self { supports_f64: true }
    }
}

#[derive(Clone, Debug)]
pub struct WgpuBackendLazy {
    device: AsyncDevice,
    queue: AsyncQueue,
    utils: WgpuComputeUtils,
    features: WgpuUsefulFeatures,
}

impl LazyBackend for WgpuBackendLazy {
    type BufferCreationError = !;
    const CHUNK_SIZE: usize = CHUNK_SIZE;
    type Utils = WgpuComputeUtils;
    type DeviceToMainBufferMapped = memory::DeviceToMainBufferMapped;
    type MainToDeviceBufferMapped = memory::MainToDeviceBufferMapped;
    type DeviceToMainBufferUnmapped = memory::DeviceToMainBufferUnmapped;
    type MainToDeviceBufferUnmapped = memory::MainToDeviceBufferUnmapped;
    type DeviceOnlyBuffer = memory::DeviceOnlyBuffer;

    fn get_utils(&self) -> &Self::Utils {
        &self.utils
    }

    fn try_create_device_only_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Result<Self::DeviceOnlyBuffer, Self::BufferCreationError> {
        Ok(memory::DeviceOnlyBuffer::make_new(
            self.clone(),
            size,
            initial_data,
        ))
    }

    fn try_create_device_to_main_memory(
        &self,
    ) -> Result<Self::DeviceToMainBufferUnmapped, Self::BufferCreationError> {
        Ok(memory::DeviceToMainBufferUnmapped::make_new(self.clone()))
    }

    fn try_create_main_to_device_memory(
        &self,
    ) -> Result<Self::MainToDeviceBufferMapped, Self::BufferCreationError> {
        Ok(memory::MainToDeviceBufferMapped::make_new(self.clone()))
    }
}

pub struct WgpuBackendConfig {
    pub buffer_ring: BufferRingConfig,
    pub backends: wgpu::Backends,
    pub allowed_features: WgpuUsefulFeatures,
}

pub type WgpuBackend = Lazy<WgpuBackendLazy>;

#[derive(Error, Debug)]
pub enum WgpuBackendError {
    #[error("no suitable device could be found")]
    SuitableDeviceNotFound,
    #[error("no suitable device could be found")]
    RingBufferError(NewBuffersError<WgpuBackendLazy>),
}

impl WgpuBackend {
    pub async fn new(
        cfg: WgpuBackendConfig,
        adapter_ranking: Option<for<'a> fn(&'a wgpu::Adapter) -> usize>,
    ) -> Result<Self, WgpuBackendError> {
        let adapter_ranking = adapter_ranking.unwrap_or(|adapter| 0);

        let instance = wgpu::Instance::new(cfg.backends);
        let adapter = instance
            .enumerate_adapters(cfg.backends)
            .sorted_by_key(adapter_ranking)
            .next()
            .ok_or(WgpuBackendError::SuitableDeviceNotFound)?;

        let features = cfg.allowed_features;
        let features = features.intersect(&adapter);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: features.as_features(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let device = AsyncDevice::new(device);
        let queue = AsyncQueue::new(device.clone(), queue);

        let utils = WgpuComputeUtils::new(device.clone());

        let res = Lazy::try_new_from(
            WgpuBackendLazy {
                features,
                device,
                queue,
                utils,
            },
            cfg.buffer_ring,
        )
        .await
        .map_err(WgpuBackendError::RingBufferError)?;

        return Ok(res);
    }
}
