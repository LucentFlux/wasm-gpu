mod compute_utils;
mod memory;

use crate::backend::lazy::buffer_ring::BufferRingConfig;
use crate::backend::lazy::{Lazy, LazyBackend};
use crate::vulkano::compute_utils::VulkanoComputeUtils;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{Device, DeviceCreateInfo, Queue, QueueCreateInfo};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::VulkanLibrary;

#[derive(Clone)]
pub struct VulkanoBackendLazy {
    device: Arc<Device>,
    queue: Arc<Queue>,
    queue_family_index: u32,
    utils: Arc<VulkanoComputeUtils>,
}

impl Debug for VulkanoBackendLazy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "vulkano backend")
    }
}

impl LazyBackend for VulkanoBackendLazy {
    const CHUNK_SIZE: usize = 1024;
    type Utils = VulkanoComputeUtils;
    type DeviceToMainBufferMapped = memory::DeviceToMainBufferMapped;
    type MainToDeviceBufferMapped = memory::MainToDeviceBufferMapped;
    type DeviceToMainBufferUnmapped = memory::DeviceToMainBufferUnmapped;
    type MainToDeviceBufferUnmapped = memory::MainToDeviceBufferUnmapped;
    type DeviceOnlyBuffer = memory::DeviceOnlyBuffer;

    fn get_utils(&self) -> &Self::Utils {
        &self.utils
    }

    fn create_device_only_memory_block(
        &self,
        size: usize,
        initial_data: Option<&[u8]>,
    ) -> Self::DeviceOnlyBuffer {
        memory::DeviceOnlyBuffer::new(self.clone(), size, initial_data)
    }

    fn create_device_to_main_memory(&self) -> Self::DeviceToMainBufferUnmapped {
        memory::DeviceToMainBufferUnmapped::new(self.clone())
    }

    fn create_main_to_device_memory(&self) -> Self::MainToDeviceBufferUnmapped {
        memory::MainToDeviceBufferUnmapped::new(self.clone())
    }
}

pub struct VulkanoBackendConfig {
    pub buffer_ring: BufferRingConfig,
}

pub type VulkanoBackend = Lazy<VulkanoBackendLazy>;

impl VulkanoBackend {
    pub async fn new(
        cfg: VulkanoBackendConfig,
        predicate_physical: Option<fn(Arc<PhysicalDevice>) -> bool>,
    ) -> Self {
        let library = VulkanLibrary::new().expect("no local Vulkan library/DLL");
        let instance = Instance::new(library, InstanceCreateInfo::default())
            .expect("failed to create instance");

        let predicate_physical = predicate_physical.unwrap_or(|_| true);

        let (queue_family_index, physical) = instance
            .enumerate_physical_devices()
            .expect("could not enumerate devices")
            .filter_map(|physical| {
                if predicate_physical(physical.clone()) {
                    let compute_index = physical
                        .queue_family_properties()
                        .iter()
                        .enumerate()
                        .position(|(_, q)| q.queue_flags.compute);
                    compute_index.map(|i| (i as u32, physical))
                } else {
                    None
                }
            })
            .next()
            .expect("no devices available");

        let (device, mut queues) = Device::new(
            physical,
            DeviceCreateInfo {
                // here we pass the desired queue family to use by index
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .expect("failed to create device");

        let queue = queues.next().unwrap();

        let utils = Arc::new(VulkanoComputeUtils::new());

        Lazy::new_from(
            VulkanoBackendLazy {
                device,
                queue,
                queue_family_index,
                utils,
            },
            cfg.buffer_ring,
        )
        .await
    }
}
