#![feature(future_join)]
#![feature(async_closure)]

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
mod vulkano;

#[cfg(test)]
pub mod tests_lib;

// Manually define our API
pub mod wasp {
    use super::*;

    // Utilities
    pub use panic_on_any::PanicOnAny;

    // Backends
    pub use crate::vulkano::VulkanoBackend;
    pub use crate::vulkano::VulkanoBackendConfig;
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
    pub use crate::store_set::builder::StoreSetBuilder;
    pub use store_set::DeviceStoreSet;
    // Instance
    pub use instance::ModuleInstanceSet;
    // Func
    pub use func::Caller;
    pub use func::Func;
}

pub use wasp::*;
