#![feature(future_join)]
#![feature(async_closure)]
#![feature(associated_type_defaults)]
#![feature(never_type)]
#![feature(unwrap_infallible)]

mod atomic_counter;
mod backend;
mod compute_utils;
mod engine;
mod externs;
mod fenwick;
mod func;
mod instance;
mod memory;
mod module;
mod panic_on_any;
mod read_only;
mod session;
mod store_set;
mod typed;
mod wgpu;

#[cfg(test)]
pub mod tests_lib;

// Manually define our API
pub mod wasp {
    use super::*;

    // Utilities
    pub use panic_on_any::PanicOnAny;

    // Backends
    pub use crate::wgpu::WgpuBackend;
    pub use crate::wgpu::WgpuBackendConfig;
    pub use backend::lazy::buffer_ring::BufferRingConfig;
    pub use backend::Backend;
    // Memory
    pub use memory::DeviceMemoryBlock;
    pub use memory::MainMemoryBlock;
    pub use memory::MemoryBlock;
    // Engine
    pub use engine::Config;
    pub use engine::Engine;
    // Module
    pub use module::Module;
    // Externs
    pub mod externs {
        pub use crate::externs::Extern;
        pub use crate::externs::NamedExtern;
    }
    // Store
    pub use store_set::builder::StoreSetBuilder;
    pub use store_set::DeviceStoreSet;
    // Instance
    pub use instance::ModuleInstanceSet;
    // Ptr
    pub use instance::func::TypedFuncPtr;
    pub use instance::func::UntypedFuncPtr;
    // Func
    pub use func::Caller;
    pub use func::Func;
}

pub use wasp::*;
