#![feature(async_closure)]

use wasm_spirv::wasp::typed::TypedMultiCallable;
use wasm_spirv::{wasp, Caller, Config, PanicOnAny};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // wgpu setup
    let instance = wgpu::Instance::new(wgpu::Backends::all());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .await
        .unwrap();

    // wasm setup
    let spirv_backend = wasp::WgpuBackend::new(device, queue, Default::default());

    let engine = wasp::Engine::new(spirv_backend, Config::default());
    let wat = r#"
        (module
            (import "host" "hello" (func $host_hello (param i32)))

            (func (export "hello")
                i32.const 3
                call $host_hello)
        )
    "#;
    let module = wasp::Module::new(&engine, wat)?;

    let mut stores = wasp::StoreSet::new(&engine, 0..10);

    let host_hello = wasp::Func::wrap(&stores, |caller: Caller<_, u32>, param: i32| {
        Box::pin(async move {
            println!("Got {} from WebAssembly", param);
            println!("my host state is: {}", caller.data());

            return Ok(());
        })
    });

    stores
        .instantiate_module(&module, &[host_hello])
        .await
        .expect_all("could not instantiate all modules");
    let hellos = stores
        .get_typed_funcs::<(), ()>("hello")
        .expect_all("could not get hello function from all instances");

    hellos
        .call_all(&mut stores, |_| ())
        .await
        .expect_all("could not call all hello functions");

    Ok(())
}
