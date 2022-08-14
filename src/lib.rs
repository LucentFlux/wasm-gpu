mod backend;
mod engine;
mod extern_imports;
mod func;
mod instance;
mod module;
mod typed;
mod wgpu;

// Manually define our API
pub mod wasp {
    use super::*;

    // Backends
    pub use backend::Backend;
    pub use wgpu::WgpuBackend;
    // Engine
    pub use engine::Config;
    pub use engine::Engine;
    // Module
    pub use module::Module;
    // Instance
    pub use instance::Instance;
    // Externs
    pub use extern_imports::Extern;
    pub mod externs {
        pub use extern_imports::Func;
        pub use extern_imports::Global;
        pub use extern_imports::Memory;
        pub use extern_imports::SharedMemory;
        pub use extern_imports::Table;
    }
}

pub use wasp::*;
