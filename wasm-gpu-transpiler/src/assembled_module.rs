mod call_graph;

use self::call_graph::CallGraph;
use crate::active_module::ActiveModule;
use crate::function_lookup::FunctionLookup;
use crate::wasm_front::FuncsInstance;
use crate::{build, BuildError, ExternalValidationError, Tuneables, ValidationError};

/// All of the functions and trampolines for a module, in wgpu objects ready to be called.
pub struct AssembledModule<'a> {
    pub module: naga::Module,
    pub module_info: naga::valid::ModuleInfo,

    // Used as debug info
    functions: FuncsInstance<'a>,
    tuneables: Tuneables,
    capabilities: naga::valid::Capabilities,
}
impl<'a> AssembledModule<'a> {
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

    fn validate<'b>(
        module: &naga::Module,
        tuneables: &Tuneables,
        capabilities: naga::valid::Capabilities,
        skip_validation: bool,
    ) -> Result<naga::valid::ModuleInfo, ValidationError> {
        // Our own sanity checks
        if module.entry_points.is_empty() {
            return Err(ValidationError::NoEntryPoints);
        }

        // spirv-tools validates for us anyway, so this just helps us get better debug info
        let flags = if !skip_validation && cfg!(debug_assertions) {
            naga::valid::ValidationFlags::all()
        } else {
            naga::valid::ValidationFlags::empty()
        };

        let info = naga::valid::Validator::new(flags, capabilities)
            .validate(&module)
            .map_err(|source| {
                ValidationError::NagaValidationError(ExternalValidationError::new(
                    source.into_inner(),
                    &module,
                    &tuneables,
                    capabilities,
                ))
            })?;

        return Ok(info);
    }

    /// Converts wasm functions to a validated naga module
    pub fn assemble(functions: FuncsInstance<'a>, tuneables: &Tuneables) -> build::Result<Self> {
        let mut module = naga::Module::default();

        let mut base_functions = FunctionLookup::empty();
        let mut stack_functions = FunctionLookup::empty();
        let mut entry_functions = FunctionLookup::empty();

        // Create active module
        let mut active_module = ActiveModule::new(&mut module, tuneables)?;

        // Calculate direct call graph to figure out if two functions are directly corecursive
        let call_graph = CallGraph::calculate(&functions);
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

                let handle = base_function.handle().clone();
                let args = base_function.get_arg_tys().clone();
                let res = base_function.get_res_ty().clone();

                (handle, args, res)
            };

            //let stack_function = stack_functions.lookup_mut(&mut active_module, &ptr);
            //populate_stack_function(&mut module, function_data, &call_order, stack_functions.lookup(&ptr.to_func_ref()))?;

            {
                let mut entry_function = entry_functions.lookup_mut(&mut active_module, &ptr);
                entry_function.populate_entry_function(base_handle, &base_args, &base_res)?;
            }
        }

        // Populate monofunctions
        brain_function.populate(&mut active_module, &stack_functions);

        Self::appease_drivers(&mut module);

        let capabilities = if !tuneables.fp_options.emulate_f64 {
            naga::valid::Capabilities::FLOAT64
        } else {
            naga::valid::Capabilities::empty()
        };

        let module_info = Self::validate(&module, tuneables, capabilities, true)
            .map_err(BuildError::ValidationError)?;

        let assembled = Self {
            module_info,
            module,
            tuneables: tuneables.clone(),
            functions,
            capabilities,
        };

        let assembled = if cfg!(feature = "opt") {
            assembled.perform_opt_passes()?
        } else {
            assembled
        };

        Ok(assembled)
    }

    /// The spirv-tools library isn't built for generated spirv, it's built for hand-coded shaders. This means
    /// that it fails to optimise some of the wierder things that we do. To get over this, we implement some of our
    /// own optimisations
    fn perform_opt_passes(self) -> build::Result<Self> {
        let Self {
            module,
            module_info: _, // Throw away old derived info
            functions,
            tuneables,
            capabilities,
        } = self;

        /*naga_map_expressions!(&mut module {
            // Hoist expressions like `(expr ? a : b) == c` to `expr ? (a == c) : (b == c)`
            (expr ? a : b) =?op c => (expr ? a =?op c : b =?op c)
        });*/

        // Reduce expressions like `expr ? val : false`, `expr ? false : val`, `expr ? val : true` or `expr ? true : val`
        // TODO

        let module_info = Self::validate(&module, &tuneables, capabilities, false)
            .map_err(BuildError::ValidationError)?;
        Ok(Self {
            module_info,
            module,
            functions,
            tuneables,
            capabilities,
        })
    }

    /// Converts our internal representation to HLSL and passes it back as a string of source code.
    ///
    /// This method is intended for debugging; the outputted source is intended to be as close as possible
    /// to the shader module that will be run, but no guarantee is made that compiling this source will give
    /// the same shader module as will be executed.
    pub fn generate_hlsl_source(&self) -> String {
        let mut output_shader = String::new();

        let hlsl_options = &crate::HLSL_OUT_OPTIONS;
        let mut writer = naga::back::hlsl::Writer::new(&mut output_shader, hlsl_options);
        writer.write(&self.module, &self.module_info).unwrap();

        return output_shader;
    }
}
