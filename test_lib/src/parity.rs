//! A collection of hand-written programs and tests that they evaluate to the expected result.
//! Uses Wasmtime as a reference implementation

use std::fmt::Debug;
use wasmtime::Trap;
use wgpu_async::wrap_to_async;
use wgpu_lazybuffers::{BufferRingConfig, MemorySystem};

pub trait ParityType {
    type WasmTimeTy: wasmtime::WasmResults + Clone + PartialEq + Debug;
    type GpuTy: wasm_types::WasmTyVec + Clone + Debug;

    fn from_gpu(val: Self::GpuTy) -> Self::WasmTimeTy;
    fn are_equal(a: Result<Self::WasmTimeTy, Trap>, b: Result<Self::GpuTy, Trap>) -> bool {
        a.eq(&b.map(Self::from_gpu))
    }
}

impl ParityType for () {
    type WasmTimeTy = ();
    type GpuTy = ();

    fn from_gpu(_val: Self::GpuTy) -> Self::WasmTimeTy {
        ()
    }
}

impl ParityType for i32 {
    type WasmTimeTy = i32;
    type GpuTy = i32;

    fn from_gpu(val: Self::GpuTy) -> Self::WasmTimeTy {
        val
    }
}

impl ParityType for i64 {
    type WasmTimeTy = i64;
    type GpuTy = i64;

    fn from_gpu(val: Self::GpuTy) -> Self::WasmTimeTy {
        val
    }
}

impl ParityType for f32 {
    type WasmTimeTy = f32;
    type GpuTy = f32;

    fn from_gpu(val: Self::GpuTy) -> Self::WasmTimeTy {
        val
    }
}

impl ParityType for f64 {
    type WasmTimeTy = f64;
    type GpuTy = f64;

    fn from_gpu(val: Self::GpuTy) -> Self::WasmTimeTy {
        val
    }
}

pub fn test_parity<Input: ParityType, Output: ParityType>(
    wasm: &str,
    target_name: &str,
    input: Input::GpuTy,
) {
    pollster::block_on(async {
        // Evaluate with wasmtime
        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::new(&engine, wasm).unwrap();
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
        let target = instance.get_func(&mut store, target_name).unwrap();
        let target = target
            .typed::<Input::WasmTimeTy, Output::WasmTimeTy>(&store)
            .unwrap();
        let truth_result = target.call(&mut store, Input::from_gpu(input.clone()));

        // Evaluate with wasm_gpu
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

        let chunk_size = 1024;
        let memory_system = MemorySystem::new(
            &device,
            BufferRingConfig {
                chunk_size,
                total_transfer_buffers: 2,
            },
        )
        .unwrap();

        let module = wasm_gpu::Module::new(
            &wasm_gpu::WasmFeatures::default(),
            wasm.as_bytes(),
            "main_module".to_owned(),
        )
        .unwrap();

        let mut store_builder = wasm_gpu::MappedStoreSetBuilder::new(
            &memory_system,
            "parity_test_storeset",
            wasm_gpu::Tuneables::default(),
        );

        let instances = store_builder
            .instantiate_module(&queue, &module, wasm_gpu::imports! {})
            .await
            .expect("could not instantiate all modules");

        let target = instances
            .get_func(target_name)
            .unwrap()
            .try_typed::<Input::GpuTy, Output::GpuTy>()
            .unwrap();

        let store_source = store_builder
            .complete(&queue)
            .await
            .expect("could not complete store builder");
        let mut stores = store_source
            .build(&memory_system, &queue, 16)
            .await
            .expect("could not build stores");

        let got_results = target
            .call_all(&memory_system, &queue, &mut stores, vec![input; 16])
            .await
            .expect("could not allocate call buffers")
            .await
            .expect("could not read results buffers");

        // Check they got the same output
        assert_eq!(got_results.len(), 16);
        let truth_result = truth_result.map_err(|err| {
            err.downcast_ref::<Trap>()
                .expect("any errors should be traps")
                .clone()
        });
        for got_result in got_results {
            let got_result = got_result.map_err(|trap_code| wasmtime::Trap::from(trap_code));

            assert!(
                Output::are_equal(truth_result.clone(), got_result.clone()),
                "expected {:?} but got {:?}",
                truth_result.clone(),
                got_result
            );
        }

        // Check their memories are the same
    })
}
