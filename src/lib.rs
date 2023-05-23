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

// Utilities
#[macro_export]
macro_rules! imports {
    (
        $(
            $module:literal : {
                $(
                    $name:literal : $ext:ident
                ),* $(,)?
            }
        ),* $(,)?
    ) => {
        vec![
            $(
                $(
                    $crate::NamedExtern {
                        module: $module.to_string(),
                        name: $name.to_string(),
                        ext: $crate::Extern::from($ext)
                    },
                )*
            )*
        ]
    };
}
pub use panic_on_any::PanicOnAny;

// Configs
pub use wasm_gpu_funcgen::FloatingPointOptions;
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
        max_bindings_per_bind_group: 11,
        max_storage_buffers_per_shader_stage: 11,
        max_compute_workgroup_size_x: 256,
        max_compute_workgroup_size_y: 1,
        max_compute_workgroup_size_z: 1,
        max_compute_workgroups_per_dimension: 256,
        max_compute_invocations_per_workgroup: 1, // Higher is *way* better
        ..wgpu::Limits::downlevel_webgl2_defaults()
    };

    //assert!(limits.check_limits(&wgpu::Limits::downlevel_defaults()));

    limits
}
