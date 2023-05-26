//! A collection of hand-written programs and tests that they evaluate to the expected result.
//! Uses Wasmtime as a reference implementation

use once_cell::sync::OnceCell;
use std::fmt::Debug;
use wasmtime::Trap;
use wgpu_async::wrap_to_async;
use wgpu_lazybuffers::{BufferRingConfig, MemorySystem};

pub trait ParityInputType:
    wasm_types::WasmTyVec + wasmtime::WasmParams + Clone + PartialEq + Debug
{
}
impl<T: wasm_types::WasmTyVec + wasmtime::WasmParams + Clone + PartialEq + Debug> ParityInputType
    for T
{
}

pub trait ParityOutputType:
    wasm_types::WasmTyVec + wasmtime::WasmResults + Clone + PartialEq + Debug
{
}
impl<T: wasm_types::WasmTyVec + wasmtime::WasmResults + Clone + PartialEq + Debug> ParityOutputType
    for T
{
}

async fn get_backend() -> (MemorySystem, wgpu_async::AsyncQueue) {
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

    let memory_system = MemorySystem::new(
        &device,
        // Low memory footprint
        BufferRingConfig {
            chunk_size: 1024,
            total_transfer_buffers: 2,
        },
    )
    .unwrap();

    return (memory_system, queue);
}

struct WgpuState {
    memory_system: MemorySystem,
    queue: wgpu_async::AsyncQueue,
}

static GPU_STATE: OnceCell<WgpuState> = OnceCell::new();
fn gpu<'a>() -> &'a WgpuState {
    GPU_STATE.get_or_init(WgpuState::new)
}

impl WgpuState {
    fn new() -> Self {
        let (memory_system, queue) = pollster::block_on(get_backend());
        Self {
            memory_system,
            queue,
        }
    }
}

pub async fn test_parity<Input: ParityInputType, Output: ParityOutputType>(
    wasm: &str,
    target_name: &str,
    input: Input,
) {
    // Evaluate with wasmtime
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let target = instance.get_func(&mut store, target_name).unwrap();
    let target = target.typed::<Input, Output>(&store).unwrap();
    let truth_result = target.call(&mut store, input.clone());

    // Evaluate with wasm_gpu
    let WgpuState {
        memory_system,
        queue,
    } = gpu();

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
        .try_typed::<Input, Output>()
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
            truth_result.clone() == got_result.clone(),
            "expected {:?} but got {:?}",
            truth_result.clone(),
            got_result
        );
    }

    // TODO: Check their memories are the same
}

pub async fn test_parity_set<Input: ParityInputType, Output: ParityOutputType>(
    wasm: &str,
    target_name: &str,
    inputs: Vec<Input>,
) {
    // Evaluate with wasmtime
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, wasm).unwrap();
    let mut truth_results = Vec::new();
    for input in inputs.clone() {
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
        let target = instance.get_func(&mut store, target_name).unwrap();
        let target = target.typed::<Input, Output>(&store).unwrap();
        let truth_result = target.call(&mut store, input.clone());
        truth_results.push(truth_result);
    }

    // Evaluate with wasm_gpu
    let WgpuState {
        memory_system,
        queue,
    } = gpu();

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
        .try_typed::<Input, Output>()
        .unwrap();

    let store_source = store_builder
        .complete(&queue)
        .await
        .expect("could not complete store builder");
    let mut stores = store_source
        .build(&memory_system, &queue, inputs.len())
        .await
        .expect("could not build stores");

    let got_results = target
        .call_all(&memory_system, &queue, &mut stores, inputs.clone())
        .await
        .expect("could not allocate call buffers")
        .await
        .expect("could not read results buffers");

    // Check they got the same output as wasmtime
    assert_eq!(got_results.len(), inputs.len());
    let truth_results: Vec<_> = truth_results
        .into_iter()
        .map(|res| {
            res.map_err(|err| {
                err.downcast_ref::<Trap>()
                    .expect("any errors should be traps")
                    .clone()
            })
        })
        .collect();
    for (got_result, truth_result) in got_results.into_iter().zip(truth_results) {
        let got_result = got_result.map_err(|trap_code| wasmtime::Trap::from(trap_code));

        assert!(
            truth_result.clone() == got_result.clone(),
            "expected {:?} but got {:?}",
            truth_result.clone(),
            got_result
        );
    }

    // TODO: Check their memories are the same
}
