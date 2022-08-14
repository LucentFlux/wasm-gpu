use crate::Backend;
use std::sync::Arc;
use wasmparser::WasmFeatures;

pub struct Config {
    pub features: WasmFeatures,
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
        }
    }
}

pub struct Engine<T>
where
    T: Backend,
{
    backend: Arc<T>,
    config: Config,
}

impl<T> Engine<T>
where
    T: Backend,
{
    pub fn new(backend: T, config: Config) -> Self {
        Self {
            backend: Arc::new(backend),
            config,
        }
    }

    pub fn config(&self) -> &Config {
        return &self.config;
    }

    pub fn backend(&self) -> Arc<T> {
        return self.backend.clone();
    }
}
