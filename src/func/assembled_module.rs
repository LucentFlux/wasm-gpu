use crate::instance::func::FuncsInstance;
use crate::module::operation::OpCode;
use crate::Tuneables;

use super::call_graph::CallGraph;

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("wasm contained an unsupported instruction")]
    UnsupportedInstructionError { instruction_opcode: OpCode },
}

/// All of the functions and trampolines for a module, in wgpu objects ready to be called.
pub struct AssembledModule {
    module: naga::Module,
    module_info: naga::valid::ModuleInfo,
}
impl AssembledModule {
    fn validate(module: &naga::Module, tuneables: &Tuneables) -> naga::valid::ModuleInfo {
        #[cfg(debug_assertions)]
        let flags = naga::valid::ValidationFlags::all();
        #[cfg(not(debug_assertions))]
        let flags = naga::valid::ValidationFlags::empty();

        let capabilities = if tuneables.hardware_supports_f64 {
            naga::valid::Capabilities::FLOAT64
        } else {
            naga::valid::Capabilities::empty()
        };
        naga::valid::Validator::new(flags, capabilities)
            .validate(&module)
            .expect("internal compile error in wasm-spirv")
    }

    pub(crate) fn assemble(
        functions: &FuncsInstance,
        tuneables: &Tuneables,
    ) -> Result<AssembledModule, BuildError> {
        let module = naga::Module::default();

        // Calculate direct call graph to figure out if two functions are directly corecursive
        let call_graph = CallGraph::calculate(functions);

        // Generate

        Ok(Self {
            module_info: Self::validate(&module, tuneables),
            module,
        })
    }

    pub(crate) fn get_module(&self) -> &naga::Module {
        &self.module
    }

    pub(crate) fn get_module_info(&self) -> &naga::valid::ModuleInfo {
        &self.module_info
    }
}
