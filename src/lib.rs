#![feature(macro_metavar_expr)]
#![feature(slice_as_chunks)]
#![feature(int_roundings)]
#![recursion_limit = "4096"]

pub const WORKGROUP_SIZE: u32 = 256;

pub const MEMORY_BINDING_INDEX: u32 = 0;
pub const MEMORY_BINDING_READ_ONLY: bool = false;
pub const MUTABLE_GLOBALS_BINDING_INDEX: u32 = 1;
pub const MUTABLE_GLOBALS_BINDING_READ_ONLY: bool = false;
pub const IMMUTABLE_GLOBALS_BINDING_INDEX: u32 = 2;
pub const IMMUTABLE_GLOBALS_BINDING_READ_ONLY: bool = true;
pub const INPUT_BINDING_INDEX: u32 = 3;
pub const INPUT_BINDING_READ_ONLY: bool = true;
pub const OUTPUT_BINDING_INDEX: u32 = 4;
pub const OUTPUT_BINDING_READ_ONLY: bool = false;
pub const STACK_BINDING_INDEX: u32 = 5;
pub const STACK_BINDING_READ_ONLY: bool = false;
pub const TABLES_BINDING_INDEX: u32 = 6;
pub const TABLES_BINDING_READ_ONLY: bool = false;
pub const DATA_BINDING_INDEX: u32 = 7;
pub const DATA_BINDING_READ_ONLY: bool = true;
pub const ELEMENTS_BINDING_INDEX: u32 = 8;
pub const ELEMENTS_BINDING_READ_ONLY: bool = true;
pub const FLAGS_BINDING_INDEX: u32 = 9;
pub const FLAGS_BINDING_READ_ONLY: bool = false;
pub const CONSTANTS_BINDING_INDEX: u32 = 10;
pub const CONSTANTS_BINDING_READ_ONLY: bool = true;

pub const BINDING_TUPLES: [(u32, bool); 11] = [
    (MEMORY_BINDING_INDEX, MEMORY_BINDING_READ_ONLY),
    (
        MUTABLE_GLOBALS_BINDING_INDEX,
        MUTABLE_GLOBALS_BINDING_READ_ONLY,
    ),
    (
        IMMUTABLE_GLOBALS_BINDING_INDEX,
        IMMUTABLE_GLOBALS_BINDING_READ_ONLY,
    ),
    (INPUT_BINDING_INDEX, INPUT_BINDING_READ_ONLY),
    (OUTPUT_BINDING_INDEX, OUTPUT_BINDING_READ_ONLY),
    (STACK_BINDING_INDEX, STACK_BINDING_READ_ONLY),
    (TABLES_BINDING_INDEX, TABLES_BINDING_READ_ONLY),
    (DATA_BINDING_INDEX, DATA_BINDING_READ_ONLY),
    (ELEMENTS_BINDING_INDEX, ELEMENTS_BINDING_READ_ONLY),
    (FLAGS_BINDING_INDEX, FLAGS_BINDING_READ_ONLY),
    (CONSTANTS_BINDING_INDEX, CONSTANTS_BINDING_READ_ONLY),
];

// Stack size is only used for recursive or co-recursive calls, and is currently fixed (and split across all instances)
pub const STACK_LEN_BYTES: u32 = 128; //268435456; // 256MB

// Flags are 32-bits wide
pub const FLAGS_LEN_BYTES: u32 = 4;
pub const TRAP_FLAG_INDEX: u32 = 0;

// Constants are 32-bits wide
pub const CONSTANTS_LEN_BYTES: u32 = 4;
pub const TOTAL_INVOCATIONS_CONSTANT_INDEX: u32 = 0;

// Strides in 4-byte words
pub const MEMORY_STRIDE_WORDS: u32 = 4;

// Alignment between single WASM value arguments when doing I/O in 4-byte words
pub const IO_ARGUMENT_ALIGNMENT_WORDS: u32 = 1;
// Alignment between sets of WASM value arguments fro each invocation when doing I/O in 4-byte words
pub const IO_INVOCATION_ALIGNMENT_WORDS: u32 = 1;

const TARGET_ENV: spirv_tools::TargetEnv = spirv_tools::TargetEnv::Vulkan_1_0;
const LANG_VERSION: (u8, u8) = (1, 0);
const HLSL_OUT_OPTIONS: naga::back::hlsl::Options = naga::back::hlsl::Options {
    shader_model: naga::back::hlsl::ShaderModel::V6_0,
    binding_map: naga::back::hlsl::BindingMap::new(),
    fake_missing_bindings: true,
    special_constants_binding: None,
    push_constants_target: None,
    zero_initialize_workgroup_memory: false,
};
const SPV_OUT_OPTIONS: naga::back::spv::Options = naga::back::spv::Options {
    lang_version: LANG_VERSION,
    flags: naga::back::spv::WriterFlags::empty(),
    binding_map: std::collections::BTreeMap::new(),
    capabilities: None, // Some(capabilities),
    bounds_check_policies: naga::proc::BoundsCheckPolicies {
        index: naga::proc::index::BoundsCheckPolicy::Unchecked,
        buffer: naga::proc::index::BoundsCheckPolicy::Unchecked,
        image: naga::proc::index::BoundsCheckPolicy::Unchecked,
        binding_array: naga::proc::index::BoundsCheckPolicy::Unchecked,
    },
    zero_initialize_workgroup_memory: naga::back::spv::ZeroInitializeWorkgroupMemoryMode::None,
};
const SPV_IN_OPTIONS: naga::front::spv::Options = naga::front::spv::Options {
    adjust_coordinate_space: false,
    strict_capabilities: false,
    block_ctx_dump_prefix: None,
};

mod active_function;
mod active_module;
mod assembled_module;
mod brain_function;
mod function_lookup;
mod std_objects;
mod traps;
mod wasm_front;

use std::error::Error;
use std::fmt::Debug;

pub use assembled_module::AssembledModule;
pub use traps::trap_to_u32;
pub use traps::u32_to_trap;
pub use wasm_front::DataIndex;
pub use wasm_front::ElementIndex;
pub use wasm_front::FuncAccessible;
pub use wasm_front::FuncData;
pub use wasm_front::FuncUnit;
pub use wasm_front::FuncsInstance;
pub use wasm_front::FunctionModuleData;
pub use wasm_front::GlobalImmutableIndex;
pub use wasm_front::GlobalIndex;
pub use wasm_front::GlobalMutableIndex;
pub use wasm_front::MemoryIndex;
pub use wasm_front::TableIndex;

#[derive(Debug, Copy, Clone)]
pub struct Tuneables {
    /// If this is true, each parallel instance is executed in its own environment
    /// and cannot see the values stored in the memories of its peers. If this is
    /// false, all instances in a set share the same block of memory, and so atomics
    /// from the threading proposal should be used by your wasm modules to ensure
    /// proper memory manipulation.
    pub disjoint_memory: bool,
    /// Which extra things to do when performing floating point operations to ensure
    /// adherance to the specification
    pub fp_options: FloatingPointOptions,
}

#[derive(Debug, Copy, Clone)]
pub struct FloatingPointOptions {
    /// Most GPUs support very fast 32-bit floating point operations, but only for some subset of 'normal' floats.
    /// WebAssembly requires SubNormals to be supported for an engine to be specification compliant. Only set this
    /// to `false` if you are 100% absolutely sure that the GPU that your program is using supports SubNormals.
    /// Setting to `false` on a GPU without SubNormal support will result in undefined/unspecified behaviour.
    pub emulate_subnormals: bool,
    /// WebGPU (and Vulkan) only require correct division when the operands are below `2^126`, which is less than the
    /// maximum 32-bit floating point value within WebAssembly. This flag emulates division when either of the operands
    /// are above this limit. Only set this to `false` if you are 100% absolutely sure that the GPU that your program
    /// is using supports division with any (non-subnormal, see `emulate_subnormals`) argument.
    /// Setting to `false` on a GPU without full-range division support will result in undefined behaviour.
    pub emulate_div_beyond_max: bool,
    /// If set to true, the translator will output f64 instructions. If false, emulated floats will be used. Similarly
    /// to other options, this should only be set to `false` if you are sure that your GPU supports 64 bit floats,
    /// however incorrect setting of this flag will result in a crash, rather than undefined/unspecified behaviour.
    pub emulate_f64: bool,
}

impl Default for Tuneables {
    fn default() -> Self {
        Self {
            disjoint_memory: true,
            fp_options: FloatingPointOptions::default(),
        }
    }
}

impl FloatingPointOptions {
    /// Use with caution; not emulating any FP operations typically results in undefined behavior on most GPUs.
    pub unsafe fn no_emulation() -> Self {
        Self {
            emulate_subnormals: false,
            emulate_div_beyond_max: false,
            emulate_f64: false,
        }
    }
}

impl Default for FloatingPointOptions {
    fn default() -> Self {
        Self {
            emulate_subnormals: true,
            emulate_div_beyond_max: true,
            emulate_f64: true,
        }
    }
}

pub fn get_entry_name(funcref: wasm_types::FuncRef) -> String {
    format!(
        "__wasm_entry_function_{}",
        funcref.as_u32().unwrap_or(u32::MAX)
    )
}

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("wasm contained an unsupported instruction {instruction_opcode:?}")]
    UnsupportedInstructionError {
        instruction_opcode: wasm_opcodes::OpCode,
    },
    #[error("wasm contained an unsupported type {wasm_type:?}")]
    UnsupportedTypeError { wasm_type: wasmparser::ValType },
    #[error("wasm had {0:?} larger than i32::MAX, and so was not addressable on the GPU's 32-bit architecture")]
    BoundsExceeded(ExceededComponent),
    #[error("naga failed to emit spir-v {0:?}")]
    NagaSpvBackError(naga::back::spv::Error),
    #[error("naga failed to receive spir-v {0:?}")]
    NagaSpvFrontError(naga::front::spv::Error),
    #[error("one of our validation checks didn't hold. This is a bug in the wasm-gpu-funcgen crate: {0:?}")]
    ValidationError(ValidationError),
}

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error("naga validation failed {0:?}")]
    NagaValidationError(ExternalValidationError<naga::valid::ValidationError>),
    #[error("spirv-tools validation failed {0:?}")]
    SpvToolsValidationError(ExternalValidationError<spirv_tools::Error>),
    #[error("the module contained no shader entry points")]
    NoEntryPoints,
    #[error("the module's binding at index {binding_index:?} for the {buffer_label:?} buffer was incompatible: got type {observed_buffer_type:?} but required type {required_buffer_type:?}")]
    IncompatableBinding {
        binding_index: u32,
        buffer_label: String,
        observed_buffer_type: naga::Type,
        required_buffer_type: naga::Type,
    },
}

#[derive(Clone)]
pub struct ExternalValidationError<E> {
    pub source: E,

    // To help with debugging this crate, we collect loads more debug info on debug builds.
    #[cfg(debug_assertions)]
    pub module: naga::Module,
    #[cfg(debug_assertions)]
    pub tuneables: Tuneables,
    #[cfg(debug_assertions)]
    pub functions: FuncsInstance,
    #[cfg(debug_assertions)]
    pub capabilities: naga::valid::Capabilities,
}

impl<E> ExternalValidationError<E> {
    fn new(
        source: E,
        module: &naga::Module,
        tuneables: &Tuneables,
        functions: &FuncsInstance,
        capabilities: naga::valid::Capabilities,
    ) -> Self {
        Self {
            source,
            #[cfg(debug_assertions)]
            module: module.clone(),
            #[cfg(debug_assertions)]
            tuneables: tuneables.clone(),
            #[cfg(debug_assertions)]
            functions: functions.clone(),
            #[cfg(debug_assertions)]
            capabilities,
        }
    }
}

pub fn display_error_recursively(error: &impl Error) -> String {
    let mut error_fmt = format! {"{}", error};
    let mut src_err: &dyn Error = error;
    while let Some(next_err) = src_err.source() {
        error_fmt = format! {"{}: {}", error_fmt, next_err};
        src_err = next_err;
    }

    return error_fmt;
}

impl<E: std::error::Error> Debug for ExternalValidationError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Output naga error
        let mut output_struct = f.debug_struct("failed to validate naga module");
        let output = output_struct.field("naga_error", &display_error_recursively(&self.source));

        if cfg!(not(feature = "big_errors")) {
            return output.finish_non_exhaustive();
        }

        // Add on lots'a debugging info
        let mut validation_pass_broken = None;
        for flag in [
            naga::valid::ValidationFlags::BINDINGS,
            naga::valid::ValidationFlags::BLOCKS,
            naga::valid::ValidationFlags::CONSTANTS,
            naga::valid::ValidationFlags::CONTROL_FLOW_UNIFORMITY,
            naga::valid::ValidationFlags::EXPRESSIONS,
            naga::valid::ValidationFlags::STRUCT_LAYOUTS,
        ] {
            let flags = flag;
            if naga::valid::Validator::new(flags, self.capabilities)
                .validate(&self.module)
                .is_err()
            {
                validation_pass_broken = Some(flag);
                break;
            }
        }

        return output
            .field("module", &self.module)
            .field("functions", &self.functions)
            .field("tuneables", &self.tuneables)
            .field("validation_pass", &validation_pass_broken)
            .finish();
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ExceededComponent {
    #[error("function return type")]
    ReturnType,
    #[error("function parameter list")]
    ParameterCount,
    #[error("memory operation argument offset size")]
    MemArgOffset,
}

pub(crate) mod build {
    use crate::BuildError;

    pub type Result<V> = std::result::Result<V, BuildError>;
}
