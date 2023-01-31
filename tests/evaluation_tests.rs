//! A collection of hand-written programs and tests that they evaluate to the expected result.
//! Uses Wasmtime as a reference implementation

use wasm_spirv::{wasp, PanicOnAny};
use wgpu_async::wrap_wgpu;
use wgpu_lazybuffers::{BufferRingConfig, MemorySystem};

fn test_parity<Res: wasmtime::WasmResults + wasp::WasmTyVal + Eq + std::fmt::Debug>(
    wasm: &str,
    target_name: &str,
) {
    pollster::block_on(async {
        // Evaluate with wasmtime
        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::new(&engine, wasm).unwrap();
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
        let target = instance.get_func(&mut store, target_name).unwrap();
        let target = target.typed::<(), Res, _>(&store).unwrap();
        let truth_result = target.call(&mut store, ()).unwrap();

        // Evaluate with Wasp
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

        let chunk_size = 1024;
        let memory_system = MemorySystem::new(
            device.clone(),
            BufferRingConfig {
                chunk_size,
                total_transfer_buffers: 2,
            },
        );

        let module = wasp::Module::new(
            &wasmparser::WasmFeatures::default(),
            wasm.as_bytes(),
            "main_module".to_owned(),
        )
        .unwrap();

        let mut store_builder =
            wasp::MappedStoreSetBuilder::new(&memory_system, wasp::Tuneables::default());

        let instances = store_builder
            .instantiate_module(&queue, &module, wasp::imports! {})
            .await
            .expect("could not instantiate all modules");

        let target = instances
            .get_func(target_name)
            .unwrap()
            .try_typed::<(), Res>()
            .unwrap();

        let store_source = store_builder
            .complete(&queue)
            .await
            .expect("could not complete store builder");
        let mut stores = store_source
            .build(&memory_system, &queue, 1)
            .await
            .expect("could not build stores");

        let got_results = target
            .call_all(&mut stores, vec![()])
            .await
            .expect_all("could not call target function");

        assert_eq!(truth_result, got_results[0]);
    })
}

#[test]
fn bare_return() {
    test_parity::<i32>(
        r#"
            (module
                (func (export "life_universe_and_everything") (result i32)
                    (i32.const 42)
                )
            )
        "#,
        "life_universe_and_everything",
    )
}
