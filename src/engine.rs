use wasmparser::WasmFeatures;
use wgpu_async::async_queue::AsyncQueue;
use wgpu_lazybuffers::MemorySystem;

pub struct Tunables {}

pub struct Config {
    pub features: WasmFeatures,
    pub tunables: Tunables,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            features: WasmFeatures {
                mutable_global: false,
                saturating_float_to_int: false,
                sign_extension: false,
                reference_types: false,
                multi_value: false,
                bulk_memory: false,
                simd: true, // Silly not to on a GPU
                relaxed_simd: false,
                threads: false,
                tail_call: false,
                deterministic_only: false,
                multi_memory: false,
                exceptions: false,
                memory64: false,
                extended_const: false,
                component_model: false,
            },
            tunables: Tunables {},
        }
    }
}

pub struct Engine {
    memory_system: MemorySystem,
    queue: AsyncQueue,
    config: Config,
}

impl Engine {
    pub fn new(memory_system: MemorySystem, queue: AsyncQueue, config: Config) -> Self {
        Self {
            memory_system,
            queue,
            config,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn memory_system(&self) -> &MemorySystem {
        &self.memory_system
    }

    pub fn queue(&self) -> &AsyncQueue {
        &self.queue
    }
}
