use itertools::Itertools;

use crate::instance::func::FuncsInstance;
use crate::module::operation::OpCode;
use crate::Tuneables;

use super::bindings_gen::BindingHandles;
use super::call_graph::CallGraph;
use super::func_gen::{make_entry_function, populate_base_function};
use super::function_collection::FunctionCollection;

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
    ) -> Result<Self, BuildError> {
        let mut module = naga::Module::default();

        // Generate bindings used for all wasm things like globals
        let bindings = BindingHandles::new(&mut module);

        // Calculate direct call graph to figure out if two functions are directly corecursive
        let call_graph = CallGraph::calculate(functions);
        let call_order = call_graph.to_call_order();

        // Generate function objects
        let refs = functions
            .all_ptrs()
            .into_iter()
            .map(|ptr| ptr.to_func_ref())
            .collect_vec();
        let base_functions = FunctionCollection::new(&mut module.functions, refs.clone());
        let stack_functions = FunctionCollection::new(&mut module.functions, refs.clone());
        let brain_function = module
            .functions
            .append(naga::Function::default(), naga::Span::UNDEFINED);

        // Populate function bodies
        for ptr in functions.all_ptrs() {
            let function_data = functions.get(&ptr);

            // Generate function bodies
            let base_handle = base_functions.lookup(&ptr.to_func_ref());
            populate_base_function(
                &mut module,
                function_data,
                &call_order,
                base_handle,
                brain_function,
                &bindings,
            )?;
            //populate_stack_function(&mut module, function_data, &call_order, stack_functions.lookup(&ptr.to_func_ref()))?;

            // Generate entry function that points into base
            make_entry_function(&mut module, ptr, base_handle, &bindings)?;
        }

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
