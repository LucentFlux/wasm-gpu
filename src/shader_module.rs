use std::borrow::Cow;

use elsa::FrozenMap;
use itertools::Itertools;
use wasm_gpu_funcgen::{get_entry_name, AssembledModule, BINDING_TUPLES};
use wasm_types::FuncRef;
use wgpu::{BindGroupLayoutDescriptor, ShaderModule};
use wgpu_async::{AsyncQueue, WgpuFuture};

use crate::session::Bindings;

pub struct WasmShaderModule {
    shader: ShaderModule,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: FrozenMap<String, Box<wgpu::ComputePipeline>>, // Lazily cache pipelines
}

impl WasmShaderModule {
    fn make_shader_module(
        device: &wgpu::Device,
        assembled: &AssembledModule,
    ) -> wgpu::ShaderModule {
        let AssembledModule { module, .. } = assembled;

        #[cfg(debug_assertions)]
        device.push_error_scope(wgpu::ErrorFilter::Validation);

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Naga(Cow::Owned(module.clone())),
        });

        // Clearly this might be slow in debug, but we assume there will be no issues in realease so drop the performance hit
        #[cfg(debug_assertions)]
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            panic!(
                "error: {:#?}\nfailed to make shader module due to naga validation error. \
                Note that the module has been validated independently of the device, \
                so this is an error caused when the valid shader was instantiated on the specific GPU. \
                This indicates that either the driver is more strict than our validation (indicating a bug in wasm-gpu), or that the cause \
                of the error is a mismatch in configuration between the device used and the shader generated, \
                for example by enabling features (e.g. 64 bit floats) within shader generation but not for the device used",
                error,
            )
        }

        return shader_module;
    }

    pub(crate) fn make(device: &wgpu::Device, assembled: &AssembledModule) -> Self {
        let shader = Self::make_shader_module(device, assembled);

        let binding_entries = BINDING_TUPLES
            .clone()
            .into_iter()
            .sorted_by_key(|(binding, _)| *binding)
            .map(|(binding, read_only)| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect_vec();
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &binding_entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        Self {
            shader,
            pipeline_layout,
            bind_group_layout,
            pipelines: FrozenMap::new(),
        }
    }

    fn ensure_pipeline_exists(&self, device: &wgpu::Device, name: &str) {
        if self.pipelines.get(name).is_some() {
            return;
        }

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&self.pipeline_layout),
            module: &self.shader,
            entry_point: name,
        });
        self.pipelines.insert(name.to_owned(), Box::new(pipeline));
    }

    pub(crate) fn run_pipeline_for_fn(
        &self,
        queue: &AsyncQueue,
        func: FuncRef,
        bindings: Bindings,
        dispatch_x: u32,
        dispatch_y: u32,
        dispatch_z: u32,
    ) -> WgpuFuture<()> {
        let device = queue.device();
        let name = get_entry_name(func);

        self.ensure_pipeline_exists(device, &name);

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let bind_group = bindings.attach(device, &self.bind_group_layout);

            let mut compute =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

            let pipeline = self
                .pipelines
                .get(&name)
                .expect("pipeline was just inserted by `ensure_pipeline_exists`");
            compute.set_pipeline(pipeline);

            compute.set_bind_group(0, &bind_group, &[]);

            compute.dispatch_workgroups(dispatch_x, dispatch_y, dispatch_z);
        }

        queue.submit([encoder.finish()])
    }
}
