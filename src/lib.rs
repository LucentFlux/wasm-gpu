#![feature(macro_metavar_expr)]
#![feature(slice_as_chunks)]

pub const MEMORY_BINDING_INDEX: u32 = 0;
pub const GLOBAL_BINDING_INDEX: u32 = 1;
pub const INPUT_BINDING_INDEX: u32 = 2;
pub const OUTPUT_BINDING_INDEX: u32 = 3;
pub const STACK_BINDING_INDEX: u32 = 4;
pub const TABLE_BINDING_INDEX: u32 = 5;
pub const DATA_BINDING_INDEX: u32 = 6;
pub const ELEMENT_BINDING_INDEX: u32 = 7;
pub const FLAGS_BINDING_INDEX: u32 = 8;

// Flags are 32-bits wide
pub const TRAP_FLAG_INDEX: u32 = 0;
pub const INVOCATION_ID_FLAG_INDEX: u32 = 1; // Set on entry

// Strides in 4-byte words
pub const MEMORY_STRIDE_WORDS: u32 = 4;

mod assembled_module;
mod bindings_gen;
mod brain_func_gen;
mod call_graph;
mod config;
mod func;
mod func_gen;
mod function_collection;
mod references;
mod std_objects;

pub use assembled_module::AssembledModule;
pub use assembled_module::BuildError;
pub use config::Tuneables;
pub use func::FuncAccessible;
pub use func::FuncData;
pub use func::FuncInstance;
pub use func::FuncUnit;
pub use func::FuncsInstance;
pub use func::FunctionModuleData;
pub use references::DataIndex;
pub use references::ElementIndex;
pub use references::GlobalImmutableIndex;
pub use references::GlobalIndex;
pub use references::GlobalMutableIndex;
pub use references::MemoryIndex;
pub use references::TableIndex;
