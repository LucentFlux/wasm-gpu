use crate::compute_utils::{Utils, WGSLSources};
use crate::wgpu::async_device::AsyncDevice;
use crate::wgpu::memory::WgpuUnmappedMemoryBlock;
use crate::WgpuBackend;
use async_trait::async_trait;
use wgpu::{Label, PipelineLayoutDescriptor, ShaderModule};

pub struct WgpuComputeUtils {
    interleave: ShaderModule,

}

impl WgpuComputeUtils {
    pub fn new(device: AsyncDevice) -> Self {
        let sources = WGSLSources::get();

        let interleave = device
            .as_ref()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Label::from("Interleave"),
                source: wgpu::ShaderSource::Wgsl(sources.interleave),
            });
        let interleave_pipeline = device.as_ref().create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Label::from("Interleave Pipeline"),
            layout: None,
            module: &interleave,
            entry_point: "main",
        });
        let bind_group_layout = interleave_pipeline.get_bind_group_layout(0);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage_buffer.as_entire_binding(),
            }],
        });

        Self { interleave }
    }
}

#[async_trait]
impl<const BUFFER_SIZE: usize> Utils<WgpuBackend<BUFFER_SIZE>> for WgpuComputeUtils {
    async fn interleave<const STRIDE: usize>(
        &self,
        src: &mut WgpuUnmappedMemoryBlock<BUFFER_SIZE>,
        dst: &mut WgpuUnmappedMemoryBlock<BUFFER_SIZE>,
        count: usize,
    ) {
        let specialized = self.interleave.
    }
}
