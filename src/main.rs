#![feature(async_closure)]

use wasm_spirv::{imports, wasp, Caller, Config, PanicOnAny};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // vulkano setup
    let conf = wasp::WgpuBackendConfig {
        buffer_ring: wasp::BufferRingConfig {
            // Minimal memory footprint for tests
            total_mem: 2 * 1024,
        },
        ..Default::default()
    };

    // wasm setup
    let spirv_backend = wasp::WgpuBackend::new(conf, None)
        .await
        .expect("failed to get wgpu instance");

    let engine = wasp::Engine::new(spirv_backend, Config::default());
    let wat = r#"
        (module
            (import "host" "hello" (func $host_hello (param i32)))

            (func (export "hello")
                i32.const 3
                call $host_hello)
        )
    "#;
    let module = wasp::Module::new(&engine, wat.as_bytes(), "main")?;

    let mut store_builder = wasp::StoreSetBuilder::new(&engine).await;

    let host_hello = wasp::Func::wrap(&mut store_builder, |caller: Caller<_, u32>, param: i32| {
        Box::pin(async move {
            println!("Got {} from WebAssembly", param);
            println!("my host state is: {}", caller.data());

            return Ok(());
        })
    });

    let instances = store_builder
        .instantiate_module(
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

    let store_source = store_builder.complete().await;
    let mut stores = store_source.build([16]).await;

    hellos
        .call_all(&mut stores, |_| ())
        .await
        .expect_all("could not call all hello functions");

    Ok(())
}
