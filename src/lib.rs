#![feature(future_join)]
#![feature(async_closure)]
#![feature(macro_metavar_expr)]
#![feature(vec_push_within_capacity)]
#![recursion_limit = "1024"]

mod atomic_counter;
mod capabilities;
mod externs;
mod func;
mod instance;
mod module;
mod panic_on_any;
mod session;
mod store_set;
mod typed;

#[cfg(test)]
pub mod tests_lib;

// Manually define our API
pub mod wasp {
    use super::*;

    // Utilities
    pub use super::imports;
    pub use panic_on_any::PanicOnAny;

    // Configs
    pub use wasm_spirv_funcgen::Tuneables;
    pub use wasmparser::WasmFeatures;
    // Module
    pub use module::Module;
    // Externs
    pub mod externs {
        pub use crate::externs::Extern;
        pub use crate::externs::NamedExtern;
    }
    // Store
    pub use store_set::builder::MappedStoreSetBuilder; // Don't need to expose the unmapped version
    pub use store_set::calling::Caller;
    pub use store_set::DeviceStoreSet;
    // Instance
    pub use instance::ModuleInstanceReferences;
    // Ptr
    pub use instance::func::TypedFuncPtr;
    pub use instance::func::UntypedFuncPtr;
    // Typing
    pub use typed::*;
}

pub use wasp::*;
