# Wasm-GPU

_Run WebAssembly on the GPU, seamlessly!™_

`Wasm-GPU` is a rust library allowing WebAssembly modules to be instantiated and executed in parallel on the GPU. 

# Getting started

To use the library, add the following to your `Cargo.toml`:

```toml
wgpu = "0.19.0"
wgpu-async = "0.19.0"
wasm-gpu = { git = "https://github.com/LucentFlux/wasm-gpu.git" }
```

`Wasm-GPU` runs using `wgpu` (and currently `wgpu-async`), so we need to first create a GPU context in which to run our modules:

```rust
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
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
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            label: None,
        },
        None,
    )
    .await
    .unwrap();

let (device, queue) = wgpu_async::wrap_to_async(device, queue);
```

Then we can use `wasm-gpu` to build and execute WebAssembly using the above GPU:

```rust
// The `MemorySystem` changes our memory footprint, describing the staging buffers used when communicating with the GPU.
let memory_system = wasm_gpu::MemorySystem::new(
    &device,
    // Low memory footprint
    BufferRingConfig {
        chunk_size: 1024,
        total_transfer_buffers: 2,
    },
)
.unwrap();

// The `Engine` holds the configuration state and handle to the GPU allowing module instances to be
// translated and instantiated. 
let mut engine = wasm_gpu::Engine::new(
    &memory_system,
    "example",
    wasm_gpu::Tuneables::default(),
);

// Initial conversion of WebAssembly to shader code is done synchronously when building a `Module`
let module = wasm_gpu::Module::new(
    &wasm_gpu::WasmFeatures::default(),
    r#"
    (module
        (func $f (result i32)
            (i32.const 42)
        )
        (export "life_universe_and_everything" (func $f))
    )
    "#.as_bytes(),
    "example_module",
)
.unwrap();

// Once instantiated on the GPU, the module data can be queried for exports such as functions to be later invoked.
let module_data = engine
    .instantiate_module(&queue, &module, wasm_gpu::imports! {})
    .await
    .unwrap();

let target = module_data
    .get_func("life_universe_and_everything")
    .unwrap()
    .try_typed::<(), i32>()
    .unwrap();

// The origin is used to instantiate a collection of instances, and can be long-lived.
// It represents the buffers and shaders allocated on the GPU before individual instances
// are spawned.
let origin = engine
    .complete(&queue)
    .await
    .unwrap();

// We can then instantiate (here 16) instances to be manipulated in parallel.
let mut instances = store_source
    .build(&memory_system, &queue, 16)
    .await
    .unwrap();

// Functions can then be invoked on the instances, and the results can be collected.
let got_results = target
    .call_all(&memory_system, &queue, &mut instances, vec![(); 16])
    .await
    .unwrap();
```
