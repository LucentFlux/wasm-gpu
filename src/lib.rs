#![feature(future_join)]
#![feature(async_closure)]

mod atomic_counter;
mod engine;
mod externs;
mod fenwick;
mod func;
mod instance;
mod module;
mod panic_on_any;
mod session;
mod store_set;
mod typed;

mod capabilities;
#[cfg(test)]
pub mod tests_lib;

// Manually define our API
pub mod wasp {
    use super::*;

    // Utilities
    pub use panic_on_any::PanicOnAny;

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
    pub use instance::ModuleInstanceReferences;
    // Ptr
    pub use instance::func::TypedFuncPtr;
    pub use instance::func::UntypedFuncPtr;
    // Func
    pub use func::Caller;
    pub use func::Func;
    // Typing
    pub use typed::*;
}

pub use wasp::*;
