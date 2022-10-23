mod compute_utils;

use crate::backend::lazy::buffer_ring::BufferRingConfig;
use crate::backend::lazy::{Lazy, LazyBackend};
use std::fmt::{Debug, Formatter};

struct VulkanoBackendLazy<const CHUNK_SIZE: usize> {}

impl<const CHUNK_SIZE: usize> Debug for VulkanoBackendLazy<CHUNK_SIZE> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "vulkano backend")
    }
}

impl<const CHUNK_SIZE: usize> LazyBackend for VulkanoBackendLazy<CHUNK_SIZE> {
    const CHUNK_SIZE: usize = CHUNK_SIZE;
    type Utils = ();
    type DeviceToMainBufferMapped = ();
    type MainToDeviceBufferMapped = ();
    type DeviceToMainBufferUnmapped = ();
    type MainToDeviceBufferUnmapped = ();
    type DeviceOnlyBuffer = ();

    fn get_utils(&self) -> &Self::Utils {
        todo!()
    }

    fn create_device_only_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Self::DeviceOnlyBuffer {
        todo!()
    }

    fn create_device_to_main_memory(&self) -> Self::DeviceToMainBufferUnmapped {
        todo!()
    }

    fn create_main_to_device_memory(&self) -> Self::DeviceToMainBufferUnmapped {
        todo!()
    }
}

pub struct VulkanoBackendConfig {
    pub(crate) buffer_ring: BufferRingConfig,
}

pub type VulkanoBackend<const CHUNK_SIZE: usize> = Lazy<VulkanoBackendLazy<CHUNK_SIZE>>;

impl<const CHUNK_SIZE: usize> VulkanoBackend<CHUNK_SIZE> {
    pub fn new(cfg: VulkanoBackendConfig) -> Self {
        Lazy::new_from(VulkanoBackendLazy {}, cfg.buffer_ring)
    }
}
