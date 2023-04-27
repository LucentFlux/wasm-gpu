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
            (func $f (param i32) (result i32)
                (local i32)
                (block
                    (block
                        (block
                            ;; x == 0
                            local.get 0
                            i32.eqz
                            br_if 0

                            ;; x == 1
                            local.get 0
                            i32.const 1
                            i32.eq
                            br_if 1

                            ;; else
                            i32.const 7
                            local.set 1
                            br 2
                        )
                        i32.const 42
                        local.set 1
                        br 1
                    )
                    i32.const 99
                    local.set 1
                )
                local.get 1
            )
            (export "foi" (func $f))
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

    let function = instances.get_func("foi").unwrap();
    let function = function.try_typed::<i32, i32>().unwrap();

    let store_source = match store_builder.complete(&queue).await {
        Ok(v) => v,
        Err(e) => panic!("could not complete store builder: {:#?}", e),
    };

    let mut stores = store_source
        .build(&memory_system, &queue, 1)
        .await
        .expect("could not build stores");

    println!("====================HLSL SHADER BEGIN====================");
    println!("{}", store_source.get_module().generate_hlsl_source());
    println!("=====================HLSL SHADER END=====================");

    let results = function
        .call_all(&memory_system, &queue, &mut stores, vec![1])
        .await
        .unwrap()
        .await
        .unwrap()
        .expect_all("could not call all functions");

    for result in results {
        println!("result: {}", result);
    }

    Ok(())
}
