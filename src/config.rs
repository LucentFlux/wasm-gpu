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
                simd: false, //TODO: Silly not to on a GPU
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
