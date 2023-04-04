#![feature(macro_metavar_expr)]
#![feature(slice_as_chunks)]
#![feature(int_roundings)]

pub const MEMORY_BINDING_INDEX: u32 = 0;
pub const MEMORY_BINDING_READ_ONLY: bool = false;
pub const MUTABLE_GLOBAL_BINDING_INDEX: u32 = 1;
pub const MUTABLE_GLOBAL_BINDING_READ_ONLY: bool = false;
pub const IMMUTABLE_GLOBAL_BINDING_INDEX: u32 = 2;
pub const IMMUTABLE_GLOBAL_BINDING_READ_ONLY: bool = true;
pub const INPUT_BINDING_INDEX: u32 = 3;
pub const INPUT_BINDING_READ_ONLY: bool = true;
pub const OUTPUT_BINDING_INDEX: u32 = 4;
pub const OUTPUT_BINDING_READ_ONLY: bool = false;
pub const STACK_BINDING_INDEX: u32 = 5;
pub const STACK_BINDING_READ_ONLY: bool = false;
pub const TABLE_BINDING_INDEX: u32 = 6;
pub const TABLE_BINDING_READ_ONLY: bool = false;
pub const DATA_BINDING_INDEX: u32 = 7;
pub const DATA_BINDING_READ_ONLY: bool = true;
pub const ELEMENT_BINDING_INDEX: u32 = 8;
pub const ELEMENT_BINDING_READ_ONLY: bool = true;
pub const FLAGS_BINDING_INDEX: u32 = 9;
pub const FLAGS_BINDING_READ_ONLY: bool = false;

pub const BINDING_TUPLES: [(u32, bool); 10] = [
    (MEMORY_BINDING_INDEX, MEMORY_BINDING_READ_ONLY),
    (
        MUTABLE_GLOBAL_BINDING_INDEX,
        MUTABLE_GLOBAL_BINDING_READ_ONLY,
    ),
    (
        IMMUTABLE_GLOBAL_BINDING_INDEX,
        IMMUTABLE_GLOBAL_BINDING_READ_ONLY,
    ),
    (INPUT_BINDING_INDEX, INPUT_BINDING_READ_ONLY),
    (OUTPUT_BINDING_INDEX, OUTPUT_BINDING_READ_ONLY),
    (STACK_BINDING_INDEX, STACK_BINDING_READ_ONLY),
    (TABLE_BINDING_INDEX, TABLE_BINDING_READ_ONLY),
    (DATA_BINDING_INDEX, DATA_BINDING_READ_ONLY),
    (ELEMENT_BINDING_INDEX, ELEMENT_BINDING_READ_ONLY),
    (FLAGS_BINDING_INDEX, FLAGS_BINDING_READ_ONLY),
];

// Stack size is only used for recursive or co-recursive calls, and is currently fixed
pub const STACK_LEN_BYTES: u32 = 1024 * 1024 * 8; // 8MB

// Flags are 32-bits wide
pub const FLAGS_LEN_BYTES: u32 = 4;
pub const TRAP_FLAG_INDEX: u32 = 0;

// Strides in 4-byte words
pub const MEMORY_STRIDE_WORDS: u32 = 4;

// Alignment between single WASM value arguments when doing I/O in 4-byte words
pub const IO_ARGUMENT_ALIGNMENT_WORDS: u32 = 1;
// Alignment between sets of WASM value arguments fro each invocation when doing I/O in 4-byte words
pub const IO_INVOCATION_ALIGNMENT_WORDS: u32 = 1;

mod assembled_module;
mod brain_func_gen;
mod call_graph;
mod config;
mod func;
mod func_gen;
mod function_collection;
mod module_ext;
mod references;
mod std_objects;
mod traps;

pub use assembled_module::AssembledModule;
pub use assembled_module::BuildError;
pub use config::Tuneables;
pub use func::FuncAccessible;
pub use func::FuncData;
pub use func::FuncInstance;
pub use func::FuncUnit;
pub use func::FuncsInstance;
pub use func::FunctionModuleData;
pub use func_gen::get_entry_name;
pub use references::DataIndex;
pub use references::ElementIndex;
pub use references::GlobalImmutableIndex;
pub use references::GlobalIndex;
pub use references::GlobalMutableIndex;
pub use references::MemoryIndex;
pub use references::TableIndex;
pub use traps::trap_to_u32;
pub use traps::u32_to_trap;
