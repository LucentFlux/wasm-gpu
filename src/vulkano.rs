mod compute_utils;

use crate::backend::lazy::{Lazy, LazyBackend};
use std::fmt::{Debug, Formatter};

struct VulkanoBackendLazy {}

impl Debug for VulkanoBackendLazy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "vulkano backend")
    }
}

impl LazyBackend for VulkanoBackendLazy {
    const CHUNK_SIZE: usize = 0;
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

pub type VulkanoBackend = Lazy<VulkanoBackendLazy>;
