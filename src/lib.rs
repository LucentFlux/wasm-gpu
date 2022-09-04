#![feature(generic_associated_types)]

mod atomic_counter;
mod backend;
mod engine;
mod extern_imports;
mod func;
mod memory;
mod module;
mod panic_on_any;
mod session;
mod store;
mod typed;
mod wgpu;

// Manually define our API
pub mod wasp {
    use super::*;

    // Utilities
    pub use panic_on_any::PanicOnAny;

    // Backends
    pub use crate::wgpu::WgpuBackend;
    pub use backend::Backend;
    // Engine
    pub use engine::Config;
    pub use engine::Engine;
    // Module
    pub use module::Module;
    // Externs
    pub use extern_imports::Extern;
    pub mod externs {
        use super::*;

        pub use extern_imports::Global;
        pub use extern_imports::Memory;
        pub use extern_imports::SharedMemory;
        pub use extern_imports::Table;
    }
    // Store
    pub use store::FuncPtr;
    pub use store::Store;
    pub use store::StoreSet;
    // Func
    pub use func::Caller;
    pub use func::Func;
    pub use func::MultiCallable;
    pub mod typed {
        use super::*;

        pub use func::TypedFuncPtr;
        pub use func::TypedMultiCallable;
    }
}

pub use wasp::*;
