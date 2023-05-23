use wasm_gpu::{imports, PanicOnAny, Tuneables};
use wasm_gpu_funcgen::FloatingPointOptions;
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
    let wat = r#"(module
    (func $f (param $x i32) (param $y i32) (param $size i32) (param $max_iterations i32) (result f32)
        (local $a f32)
        (local $b f32)
        (local $x_0 f32)
        (local $y_0 f32)
        (local $iterations i32)

        ;; a = 0.0
        f32.const 0.0
        local.set $a
        ;; b = 0.0
        f32.const 0.0
        local.set $b

        ;; x_0 = (x / size) * 4.0 - 2.0
        local.get $x
        f32.convert_i32_s
        local.get $size
        f32.convert_i32_s
        f32.div
        f32.const 4.0
        f32.mul
        f32.const 2.0
        f32.sub
        local.set $x_0

        ;; y_0 = (y / size) * 4.0 - 2.0
        local.get $y
        f32.convert_i32_s
        local.get $size
        f32.convert_i32_s
        f32.div
        f32.const 4.0
        f32.mul
        f32.const 2.0
        f32.sub
        local.set $y_0

        ;; iterations = -1
        i32.const -1
        local.set $iterations

        (loop $inner
            ;; a_new = a * a - b * b + x0
            local.get $a
            local.get $a
            f32.mul

            local.get $b
            local.get $b
            f32.mul

            f32.sub

            local.get $x_0
            f32.add

            ;; b_new = 2.0 * a * b + y0
            f32.const 2.0
            local.get $a
            f32.mul
            local.get $b
            f32.mul

            local.get $y_0
            f32.add

            ;; a = a_new; b = b_new
            local.set $b
            local.set $a

            ;; iterations += 1
            local.get $iterations
            i32.const 1
            i32.add
            local.set $iterations

            ;; loop while iterations < max_iterations && a * a + b * b <= 4.0
            local.get $iterations
            local.get $max_iterations
            i32.lt_s

            local.get $a
            local.get $a
            f32.mul

            local.get $b
            local.get $b
            f32.mul

            f32.add

            f32.const 4.0

            f32.le

            i32.and

            br_if $inner
        )

        local.get $iterations
        f32.convert_i32_s
        local.get $max_iterations
        f32.convert_i32_s
        f32.div
    )
    (export "foi" (func $f))
)
        "#;
    let module = wasm_gpu::Module::new(
        &wasmparser::WasmFeatures::default(),
        wat.as_bytes(),
        "main_module".to_owned(),
    )?;

    let mut store_builder = wasm_gpu::MappedStoreSetBuilder::new(
        &memory_system,
        "main_store",
        Tuneables {
            disjoint_memory: true,
            fp_options: FloatingPointOptions {
                emulate_subnormals: true,
                emulate_div_beyond_max: true,
                emulate_f64: true,
            },
        },
    );

    let instances = store_builder
        .instantiate_module(&queue, &module, imports! {})
        .await
        .expect("could not instantiate all modules");

    let function = instances.get_func("foi").unwrap();
    let function = function.try_typed::<(i32, i32, i32, i32), f32>().unwrap();

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
        .call_all(
            &memory_system,
            &queue,
            &mut stores,
            vec![(100, 100, 200, 200)],
        )
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
