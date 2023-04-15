use wasm_gpu::{imports, PanicOnAny, Tuneables};
use wgpu_async::wrap_to_async;
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

    let (device, queue) = wrap_to_async(device, queue);

    let chunk_size = 16 * 1024;
    let memory_system = MemorySystem::new(
        &device,
        BufferRingConfig {
            chunk_size,
            total_transfer_buffers: 1024,
        },
    )
    .unwrap();

    // wasm setup
    let wat = r#"
        (module
            (func $f (param f64) (result f64)
                local.get 0
            )
            (export "pass_f64" (func $f))
        )
        "#;
    let module = wasm_gpu::Module::new(
        &wasmparser::WasmFeatures::default(),
        wat.as_bytes(),
        "main_module".to_owned(),
    )?;

    let mut store_builder =
        wasm_gpu::MappedStoreSetBuilder::new(&memory_system, "main_store", Tuneables::default());

    let instances = store_builder
        .instantiate_module(&queue, &module, imports! {})
        .await
        .expect("could not instantiate all modules");

    let function = instances.get_func("pass_f64").unwrap();
    let function = function.try_typed::<f64, f64>().unwrap();

    let store_source = store_builder
        .complete(&queue)
        .await
        .expect("could not complete store builder");

    let mut stores = store_source
        .build(&memory_system, &queue, 1)
        .await
        .expect("could not build stores");

    println!("====================UNOPTIMIZED HLSL SHADER BEGIN====================");
    println!("{}", store_source.generate_hlsl_source());
    println!("=====================UNOPTIMIZED HLSL SHADER END=====================");

    println!("====================OPTIMIZED HLSL SHADER BEGIN====================");
    println!("{}", store_source.generate_optimised_hlsl_source().unwrap());
    println!("=====================OPTIMIZED HLSL SHADER END=====================");

    let results = function
        .call_all(&memory_system, &queue, &mut stores, vec![1000.01f64])
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
