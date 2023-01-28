use crate::instance::func::FuncsInstance;
use crate::module::operation::OpCode;

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("wasm contained an unsupported instruction")]
    UnsupportedInstructionError { instruction_opcode: OpCode },
}

/// All of the functions and trampolines for a module, in wgpu objects ready to be called.
pub struct AssembledModule {
    module: naga::Module,
}
impl AssembledModule {
    pub(crate) fn assemble<T>(functions: &FuncsInstance<T>) -> Result<AssembledModule, BuildError> {
        let module = naga::Module::default();

        Ok(Self { module })
    }
}
