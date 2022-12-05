use lib_hal::backend::Backend;
use std::sync::Arc;
use wasmparser::WasmFeatures;

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

pub struct Engine<B>
where
    B: Backend,
{
    backend: Arc<B>,
    config: Config,
}

impl<B> Engine<B>
where
    B: Backend,
{
    pub fn new(backend: B, config: Config) -> Self {
        Self {
            backend: Arc::new(backend),
            config,
        }
    }

    pub fn config(&self) -> &Config {
        return &self.config;
    }

    pub fn backend(&self) -> Arc<B> {
        return self.backend.clone();
    }
}
