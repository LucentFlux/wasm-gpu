use std::{borrow::Cow, collections::BTreeMap};

use elsa::FrozenMap;
use itertools::Itertools;
use std::collections::HashSet;
use wasm_gpu_funcgen::Tuneables;
use wasm_gpu_funcgen::{get_entry_name, AssembledModule, BINDING_TUPLES};
use wasm_gpu_funcgen::{
    DATA_BINDING_INDEX, ELEMENT_BINDING_INDEX, FLAGS_BINDING_INDEX, IMMUTABLE_GLOBAL_BINDING_INDEX,
    INPUT_BINDING_INDEX, MEMORY_BINDING_INDEX, MUTABLE_GLOBAL_BINDING_INDEX, OUTPUT_BINDING_INDEX,
    STACK_BINDING_INDEX, TABLE_BINDING_INDEX,
};
use wasm_types::FuncRef;
use wgpu::{BindGroupLayoutDescriptor, ShaderModule};
use wgpu_async::{AsyncQueue, WgpuFuture};

#[cfg(feature = "opt")]
use spirv_tools::opt::Optimizer;

pub struct Bindings<'a> {
    pub data: &'a wgpu::Buffer,
    pub element: &'a wgpu::Buffer,
    pub mutable_globals: &'a wgpu::Buffer,
    pub immutable_globals: &'a wgpu::Buffer,
    pub memory: &'a wgpu::Buffer,
    pub table: &'a wgpu::Buffer,

    pub flags: &'a wgpu::Buffer,
    pub input: &'a wgpu::Buffer,
    pub output: &'a wgpu::Buffer,
    pub stack: &'a wgpu::Buffer,
}

impl<'a> Bindings<'a> {
    fn attach(&self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: DATA_BINDING_INDEX,
                    resource: self.data.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: ELEMENT_BINDING_INDEX,
                    resource: self.element.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: MUTABLE_GLOBAL_BINDING_INDEX,
                    resource: self.mutable_globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: IMMUTABLE_GLOBAL_BINDING_INDEX,
                    resource: self.immutable_globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: MEMORY_BINDING_INDEX,
                    resource: self.memory.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: TABLE_BINDING_INDEX,
                    resource: self.table.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: FLAGS_BINDING_INDEX,
                    resource: self.flags.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: INPUT_BINDING_INDEX,
                    resource: self.input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: OUTPUT_BINDING_INDEX,
                    resource: self.output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: STACK_BINDING_INDEX,
                    resource: self.stack.as_entire_binding(),
                },
            ],
        })
    }

    /// Used because bindings can't be empty in wgpu
    pub(crate) fn ensure_none_empty(&mut self, non_empty: &'a wgpu::Buffer) {
        if self.data.size() == 0 {
            self.data = non_empty
        }
        if self.element.size() == 0 {
            self.element = non_empty
        }
        if self.flags.size() == 0 {
            self.flags = non_empty
        }
        if self.immutable_globals.size() == 0 {
            self.immutable_globals = non_empty
        }
        if self.mutable_globals.size() == 0 {
            self.mutable_globals = non_empty
        }
        if self.input.size() == 0 {
            self.input = non_empty
        }
        if self.memory.size() == 0 {
            self.memory = non_empty
        }
        if self.output.size() == 0 {
            self.output = non_empty
        }
        if self.stack.size() == 0 {
            self.stack = non_empty
        }
        if self.table.size() == 0 {
            self.table = non_empty
        }
    }
}

pub struct WasmShaderModule {
    shader: ShaderModule,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: FrozenMap<String, Box<wgpu::ComputePipeline>>, // Lazily cache pipelines
}

impl WasmShaderModule {
    fn make_naga_shader_module(
        device: &wgpu::Device,
        assembled: &AssembledModule,
    ) -> wgpu::ShaderModule {
        let AssembledModule { module, .. } = assembled;
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Naga(Cow::Owned(module.clone())),
        })
    }

    #[cfg(feature = "opt")]
    fn make_spv_shader_module(
        device: &wgpu::Device,
        assembled: &AssembledModule,
        tuneables: &Tuneables,
    ) -> anyhow::Result<wgpu::ShaderModule> {
        let (target_env, lang_version) = (spirv_tools::TargetEnv::Vulkan_1_0, (1, 0));

        let mut opt = spirv_tools::opt::create(Some(target_env));
        opt.register_performance_passes();

        let mut capabilities = naga::FastHashSet::from_iter([naga::back::spv::Capability::Kernel]);
        if tuneables.hardware_supports_f64 {
            capabilities.insert(naga::back::spv::Capability::Float64);
        }

        let module = &assembled.module;
        let module_info = &assembled.module_info;
        let hlsl_options = naga::back::spv::Options {
            lang_version,
            flags: naga::back::spv::WriterFlags::empty(),
            binding_map: BTreeMap::new(),
            capabilities: Some(capabilities),
            bounds_check_policies: naga::proc::BoundsCheckPolicies {
                index: naga::proc::index::BoundsCheckPolicy::Unchecked,
                buffer: naga::proc::index::BoundsCheckPolicy::Unchecked,
                image: naga::proc::index::BoundsCheckPolicy::Unchecked,
                binding_array: naga::proc::index::BoundsCheckPolicy::Unchecked,
            },
            zero_initialize_workgroup_memory:
                naga::back::spv::ZeroInitializeWorkgroupMemoryMode::None,
        };
        let mut writer =
            naga::back::spv::Writer::new(&hlsl_options).map_err(anyhow::Error::from)?;

        let mut words = Vec::new();
        writer
            .write(module, module_info, None, &mut words)
            .map_err(anyhow::Error::from)?;

        let optimised = opt
            .optimize(
                words,
                &mut |message| println!("spirv-opt message: {:?}", message),
                None,
            )
            .map_err(anyhow::Error::from)?;

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::SpirV(optimised.as_words().into()),
        });

        Ok(module)
    }

    #[cfg(feature = "opt")]
    fn make_shader_module(
        device: &wgpu::Device,
        assembled: &AssembledModule,
        tuneables: &Tuneables,
    ) -> wgpu::ShaderModule {
        match Self::make_spv_shader_module(device, assembled, tuneables) {
            Ok(writer) => writer,
            // Fall back to naga
            Err(e) => {
                debug_assert!(false, "failed to create optimised spir-v: {:?}", e);
                return Self::make_naga_shader_module(device, assembled);
            }
        }
    }

    #[cfg(not(feature = "opt"))]
    fn make_shader_module(
        device: &wgpu::Device,
        assembled: &AssembledModule,
        _tuneables: &Tuneables,
    ) -> wgpu::ShaderModule {
        Self::make_naga_shader_module(device, assembled)
    }

    pub(crate) fn make(
        device: &wgpu::Device,
        assembled: &AssembledModule,
        tuneables: &Tuneables,
    ) -> Self {
        let shader = Self::make_shader_module(device, assembled, tuneables);

        let binding_entries = BINDING_TUPLES
            .clone()
            .into_iter()
            .map(|(binding, _read_only)| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false }, // TODO: Figure out why this isn't allowed to be passed through
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
