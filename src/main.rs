use wasm_spirv::{imports, wasp, PanicOnAny, Tuneables};
use wgpu_async::wrap_wgpu;
use wgpu_lazybuffers::{BufferRingConfig, MemorySystem};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits: adapter.limits(),
                label: None,
            },
            None,
        )
        .await
        .unwrap();

    let (device, queue) = wrap_wgpu(device, queue);

    let chunk_size = 16 * 1024;
    let memory_system = MemorySystem::new(
        device.clone(),
        BufferRingConfig {
            chunk_size,
            total_transfer_buffers: 1024,
        },
    );

    // wasm setup
    let wat = r#"
        (module
            (func $f (result i32)
                (i32.const 42)
            )
            (export "fill_with_value" (func $f))
        )
    "#;
    let module = wasp::Module::new(
        &wasmparser::WasmFeatures::default(),
        wat.as_bytes(),
        "main_module".to_owned(),
    )?;

    let mut store_builder = wasp::MappedStoreSetBuilder::new(&memory_system, Tuneables::default());

    let instances = store_builder
        .instantiate_module(&queue, &module, imports! {})
        .await
        .expect("could not instantiate all modules");

    let function = instances.get_func("fill_with_value").unwrap();
    let function = function.try_typed::<(), i32>().unwrap();

    let store_source = store_builder
        .complete(&queue)
        .await
        .expect("could not complete store builder");

    let mut stores = store_source
        .build(&memory_system, &queue, 1)
        .await
        .expect("could not build stores");

    let module = &store_source.get_module();
    let module_info = &store_source.get_module_info();
    let mut output_shader = String::new();
    let hlsl_options = naga::back::hlsl::Options {
        shader_model: naga::back::hlsl::ShaderModel::V6_0,
        binding_map: naga::back::hlsl::BindingMap::new(),
        fake_missing_bindings: true,
        special_constants_binding: None,
        push_constants_target: None,
        zero_initialize_workgroup_memory: false,
    };
    let mut writer = naga::back::hlsl::Writer::new(&mut output_shader, &hlsl_options);
    writer.write(module, module_info).unwrap();
    println!("====================HLSL SHADER BEGIN====================");
    println!("{}", output_shader);
    println!("=====================HLSL SHADER END=====================");

    let results = function
        .call_all(&memory_system, &queue, &mut stores, vec![()])
        .await
        .unwrap()
        .await
        .unwrap()
        .expect_all("could not call all hello functions");

    for result in results {
        println!("result: {}", result);
    }

    Ok(())
}
