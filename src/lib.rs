#![feature(generic_associated_types)]
#![feature(async_closure)]
#![feature(macro_metavar_expr)]
#![feature(associated_type_defaults)]
#![feature(future_join)]

extern crate core;

mod atomic_counter;
mod backend;
mod engine;
mod externs;
mod func;
mod global_instance;
mod instance;
mod memory;
mod module;
mod panic_on_any;
mod read_only;
mod session;
mod store;
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
    pub use crate::wgpu::BufferRingConfig;
    pub use crate::wgpu::WgpuBackend;
    pub use crate::wgpu::WgpuBackendConfig;
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
    pub use externs::Extern;
    // Store
    pub use store::builder::StoreSetBuilder;
    pub use store::FuncPtr;
    pub use store::Store;
    pub use store::StoreSet;
    // Instance
    pub use instance::InstanceSet;
    pub use instance::ModuleInstance;
    // Func
    pub use func::Caller;
    pub use func::Func;
    pub use func::FuncSet;
    pub mod typed {
        use super::*;

        pub use func::TypedFuncPtr;
    }
}

pub use wasp::*;
