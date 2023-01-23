use wasm_spirv::{imports, wasp, Config, PanicOnAny};
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
            (import "host" "hello" (func $host_hello (param i32)))

            (func (export "hello")
                i32.const 3
                call $host_hello)
        )
    "#;
    let config = Config::default();
    let module = wasp::Module::new(&config, wat.as_bytes())?;

    let mut store_builder = wasp::MappedStoreSetBuilder::new(&memory_system);

    let host_hello = wasp::Func::wrap(
        &mut store_builder,
        |caller: wasp::Caller<u32>, param: i32| {
            Box::pin(async move {
                println!("Got {} from WebAssembly", param);
                println!("my host state is: {}", caller.data());

                return Ok(());
            })
        },
    );

    let instances = store_builder
        .instantiate_module(
            &memory_system,
            &queue,
            &module,
            imports! {
                "host": {
                    "hello": host_hello
                }
            },
        )
        .await
        .expect("could not instantiate all modules");
    let hellos = instances
        .get_typed_func::<(), ()>("hello")
        .expect("could not get hello function from all instances");

    let store_source = store_builder
        .complete(&queue)
        .await
        .expect("could not complete store builder");
    let mut stores = store_source
        .build(&memory_system, &queue, [16])
        .await
        .expect("could not build stores");

    hellos
        .call_all(&mut stores, vec![()])
        .await
        .expect_all("could not call all hello functions");

    Ok(())
}
