use wasm_spirv::{imports, wasp, PanicOnAny, Tuneables};
use wgpu_async::wrap_wgpu;
use wgpu_lazybuffers::{BufferRingConfig, MemorySystem};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let instance = wgpu::Instance::new(wgpu::Backends::all());
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
            (func (result i32)
                (i32.const 42)
            )
        )
    "#;
    let module = wasp::Module::new(
        &wasmparser::WasmFeatures::default(),
        wat.as_bytes(),
        "main_module".to_owned(),
    )?;

    let mut store_builder =
        wasp::MappedStoreSetBuilder::<()>::new(&memory_system, Tuneables::default());

    /*let host_hello =
    store_builder.register_host_function(|caller: wasp::Caller<u32>, param: i32| {
        Box::pin(async move {
            println!("Got {} from WebAssembly", param);
            println!("my host state is: {}", caller.data());

            return Ok(());
        })
    });*/

    let instances = store_builder
        .instantiate_module(
            &queue,
            &module,
            imports! {
                /*"host": {
                    "hello": host_hello
                }*/
            },
        )
        .await
        .expect("could not instantiate all modules");
    /*let hellos = instances
    .get_typed_func::<(), ()>("hello")
    .expect("could not get hello function from all instances");*/

    let store_source = store_builder
        .complete(&queue)
        .await
        .expect("could not complete store builder");

    // Get generated source
    let module = store_source.get_module();
    let module_info = store_source.get_module_info();
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

    /*let mut stores = store_source
        .build(&memory_system, &queue, [16])
        .await
        .expect("could not build stores");

    hellos
        .call_all(&mut stores, vec![()])
        .await
        .expect_all("could not call all hello functions");*/

    Ok(())
}
