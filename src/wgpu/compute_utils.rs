use crate::compute_utils::{Utils, WGSLSources};
use crate::wgpu::async_device::AsyncDevice;
use crate::WgpuBackend;
use async_trait::async_trait;
use wgpu::{Label, ShaderModule};

pub struct WgpuComputeUtils {
    interleave: ShaderModule,
}

impl WgpuComputeUtils {
    pub async fn new(device: AsyncDevice) -> Self {
        let sources = WGSLSources::get();

        let interleave = device
            .as_ref()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Label::from("Interleave"),
                source: wgpu::ShaderSource::Wgsl(sources.interleave),
            });
        Self { interleave }
    }
}

#[async_trait]
impl Utils<WgpuBackend> for WgpuComputeUtils {
    async fn interleave<const STRIDE: usize>(
        &mut self,
        src: WgpuDeviceMemoryBlock,
        dst: WgpuDeviceMemoryBlock,
        count: usize,
    ) {
        unimplemented!()
    }
}
