#![feature(future_join)]
#![feature(async_closure)]
#![feature(vec_push_within_capacity)]
#![feature(int_roundings)]

mod atomic_counter;
mod capabilities;
mod externs;
mod func;
mod instance;
mod module;
mod panic_on_any;
mod session;
mod shader_module;
mod store_set;
mod typed;

#[cfg(test)]
mod unit_tests_lib;

// Manually define our API
mod wasm_gpu {
    use super::*;

    // Utilities
    pub use super::imports;
    pub use panic_on_any::PanicOnAny;

    // Configs
    pub use wasm_gpu_funcgen::Tuneables;
    pub use wasmparser::WasmFeatures;
    // Module
    pub use module::Module;
    // Externs
    pub use crate::externs::Extern;
    pub use crate::externs::NamedExtern;
    // Store
    pub use store_set::builder::MappedStoreSetBuilder; // Don't need to expose the unmapped version
    pub use store_set::DeviceStoreSet;
    // Instance
    pub use instance::ModuleInstanceReferences;
    // Ptr
    pub use instance::func::TypedFuncPtr;
    pub use instance::func::UntypedFuncPtr;
    // Typing
    pub use typed::*;

    // Constants
    /// The limits required for evaluating wasm on the gpu.
    pub fn downlevel_wasm_defaults() -> wgpu::Limits {
        let limits = wgpu::Limits {
            max_bindings_per_bind_group: 10,
            max_storage_buffers_per_shader_stage: 10,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        };

        //assert!(limits.check_limits(&wgpu::Limits::downlevel_defaults()));

        limits
    }
}

pub use wasm_gpu::*;
