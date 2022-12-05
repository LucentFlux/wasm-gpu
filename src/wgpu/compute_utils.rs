use crate::compute_utils::{Utils, WGSLSources};
use crate::lazy_small_map::LazySmallMap;
use crate::wgpu::async_device::AsyncDevice;
use crate::{Backend, WgpuBackend};
use async_trait::async_trait;
use std::borrow::Cow;
use wgpu::{BindGroupLayout, Label, ShaderModule};

#[derive(Debug)]
struct ModuleInfo {
    pub module: ShaderModule,
    pub bind_group_layout: BindGroupLayout,
}

impl ModuleInfo {
    fn new(source: &str, device: &AsyncDevice, name: &str) -> Self {
        let module = device
            .as_ref()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Label::from(name),
                source: wgpu::ShaderSource::Glsl {
                    shader: Cow::from(source),
                    stage: naga::ShaderStage::Compute,
                    defines: Default::default(),
                },
            });
        let pipeline = device
            .as_ref()
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(&format!("{} Pipeline", name)),
                layout: None,
                module: &module,
                entry_point: "main",
            });
        let bind_group_layout = pipeline.get_bind_group_layout(0);

        Self {
            module,
            bind_group_layout,
        }
    }
}

#[derive(Debug)]
pub struct WgpuComputeUtils {
    interleave_modules: LazySmallMap<usize, ModuleInfo>,
    device: AsyncDevice,
}

impl WgpuComputeUtils {
    pub fn new(device: AsyncDevice) -> Self {
        Self {
            device,
            interleave_modules: LazySmallMap::empty(),
        }
    }
}

#[async_trait]
impl Utils<WgpuBackend> for WgpuComputeUtils {
    async fn interleave<const STRIDE: usize>(
        &self,
        src: &<WgpuBackend as Backend>::DeviceMemoryBlock,
        dst: &mut <WgpuBackend as Backend>::DeviceMemoryBlock,
        count: usize,
    ) {
        let module = self.interleave_modules.get_or_create(STRIDE, || {
            let source = WGSLSources::get_interleave_source::<{ STRIDE }>();
            ModuleInfo::new(source.as_str(), &self.device, "Interleave")
        });

        let bind_group = self
            .device
            .as_ref()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &module.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: src.data.buffer.raw_buffer().as_ref().as_entire_binding(),
                }],
            });
    }
}
