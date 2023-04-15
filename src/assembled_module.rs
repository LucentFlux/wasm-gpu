mod call_graph;

use self::call_graph::CallGraph;
use crate::function_lookup::FunctionLookup;
use std::error::Error;

use naga::valid::ValidationError;
use naga::WithSpan;

use crate::active_module::ActiveModule;
use crate::wasm_front::FuncsInstance;
use crate::{build, BuildError, Tuneables};

/// All of the functions and trampolines for a module, in wgpu objects ready to be called.
pub struct AssembledModule {
    pub module: naga::Module,
    pub module_info: naga::valid::ModuleInfo,
}
impl AssembledModule {
    /// Some drivers don't like some edge cases. To avoid driver crashes or panics, several modifications
    /// *that don't change module semantics* are employed here.
    fn appease_drivers(module: &mut naga::Module) {
        if module.entry_points.is_empty() {
            // Shaders must do something, even if our module doesn't. Introduce a dud function
            // that does nothing and that isn't exposed to the outside world
            module.entry_points.push(naga::EntryPoint {
                name: "dud_entry".to_owned(),
                stage: naga::ShaderStage::Compute,
                early_depth_test: None,
                workgroup_size: [1, 1, 1],
                function: naga::Function::default(),
            })
        }
    }

    fn validate(
        module: &naga::Module,
        tuneables: &Tuneables,
        functions: &FuncsInstance,
        // True on debug, or if the module comes from outside this crate
        validate_all: bool,
    ) -> build::Result<naga::valid::ModuleInfo> {
        // Our own sanity checks
        if module.entry_points.is_empty() {
            return Err(BuildError::ValidationError(
                crate::ValidationError::NoEntryPoints,
            ));
        }

        let flags = if validate_all {
            naga::valid::ValidationFlags::all()
        } else {
            naga::valid::ValidationFlags::empty()
        };

        let capabilities = if tuneables.hardware_supports_f64 {
            naga::valid::Capabilities::FLOAT64
        } else {
            naga::valid::Capabilities::empty()
        };
        let info = naga::valid::Validator::new(flags, capabilities)
            .validate(&module)
            .map_err(|source| {
                BuildError::ValidationError(crate::ValidationError::NagaValidationError(
                    crate::NagaValidationError {
                        source: source.into_inner(),
                        #[cfg(debug_assertions)]
                        module: module.clone(),
                        #[cfg(debug_assertions)]
                        tuneables: tuneables.clone(),
                        #[cfg(debug_assertions)]
                        functions: functions.clone(),
                        #[cfg(debug_assertions)]
                        capabilities,
                    },
                ))
            })?;

        return Ok(info);
    }

    /// Converts wasm functions to a validated naga module
    pub fn assemble(functions: &FuncsInstance, tuneables: &Tuneables) -> build::Result<Self> {
        let mut module = naga::Module::default();

        let mut base_functions = FunctionLookup::empty();
        let mut stack_functions = FunctionLookup::empty();
        let mut entry_functions = FunctionLookup::empty();

        // Create active module
        let mut active_module = ActiveModule::new(&mut module, tuneables)?;

        // Calculate direct call graph to figure out if two functions are directly corecursive
        let call_graph = CallGraph::calculate(functions);
        let call_order = call_graph.to_call_order();

        // Declare base and entry functions first
        for ptr in call_order.get_in_order() {
            let function_data = functions
                .get(*ptr)
                .expect("call order doesn't invent functions");
            let base_function = active_module.declare_base_function(*ptr, function_data)?;
            base_functions.insert(*ptr, base_function);
            let entry_function = active_module.declare_entry_function(*ptr);
            entry_functions.insert(*ptr, entry_function);
        }

        // Declare brain function
        let brain_function = active_module.declare_brain_function();

        // Declare recursive functions after brain
        for ptr in call_order.get_in_order() {
            let function_data = functions
                .get(*ptr)
                .expect("call order doesn't invent functions");
            let stack_function = active_module.declare_stack_function(*ptr, function_data)?;
            stack_functions.insert(*ptr, stack_function);
        }

        // Populate functions
        for (ptr, function_data) in functions.all_items() {
            let (base_handle, base_args, base_res) = {
                let mut base_function = base_functions.lookup_mut(&mut active_module, &ptr);
                base_function.populate_base_function(function_data)?;

                let handle = base_function.handle();
                let args = base_function.get_arg_tys().clone();
                let res = base_function.get_res_ty().clone();
                (handle, args, res)
            };

            //let stack_function = stack_functions.lookup_mut(&mut active_module, &ptr);
            //populate_stack_function(&mut module, function_data, &call_order, stack_functions.lookup(&ptr.to_func_ref()))?;

            let mut entry_function = entry_functions.lookup_mut(&mut active_module, &ptr);
            entry_function.populate_entry_function(base_handle, &base_args, &base_res)?;
        }

        // Populate monofunctions
        brain_function.populate(&mut active_module, &stack_functions);

        Self::appease_drivers(&mut module);

        Ok(Self {
            module_info: Self::validate(&module, tuneables, functions, cfg!(debug_assert))?,
            module,
        })
    }

    /// Takes an arbitrary naga module and validates that it can be fed into the wasm-gpu engine, i.e. that
    /// all of the correct bindings exist and that the module is a correctly typed module.
    pub fn from_module(
        module: naga::Module,
        functions: &FuncsInstance,
        tuneables: &Tuneables,
    ) -> build::Result<Self> {
        Ok(Self {
            module_info: Self::validate(&module, tuneables, functions, true)?,
            module,
        })
    }
}
