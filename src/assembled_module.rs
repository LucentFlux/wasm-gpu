mod call_graph;

use self::call_graph::CallGraph;
use crate::function_lookup::FunctionLookup;
use std::error::Error;

use naga::valid::ValidationError;
use naga::WithSpan;

use crate::active_module::ActiveModule;
use crate::std_objects::{FullPolyfill, StdObjects};
use crate::wasm_front::FuncsInstance;
use crate::{build, Tuneables};

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

    /// Invokes panic!, but with lots of debugging information about this module (on debug)
    fn validation_panic(
        err: WithSpan<ValidationError>,
        module: &naga::Module,
        tuneables: &Tuneables,
        functions: &FuncsInstance,
        capabilities: naga::valid::Capabilities,
    ) -> ! {
        let mut err_display = format! {"{}", err};
        let mut src_err: &dyn Error = &err;
        while let Some(next_err) = src_err.source() {
            err_display = format! {"{}: {}", err_display, next_err};
            src_err = next_err;
        }

        #[cfg(not(debug_assertions))]
        panic!(
            "failed to validate wasm-generated naga module: {}",
            err_display
        );

        // Lots'a debugging info
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
            if naga::valid::Validator::new(flags, capabilities)
                .validate(module)
                .is_err()
            {
                validation_pass_broken = Some(flag);
                break;
            }
        }
        panic!(
            "failed to validate wasm-generated naga module in pass {:?}: {}\n{{\nnaga_error: {:#?},\nnaga module: {:#?},\nwasm functions: {:#?},\ntuneables: {:#?}\n}}",
            validation_pass_broken, err_display, err, module, functions, tuneables
        )
    }

    fn validate(
        module: &naga::Module,
        tuneables: &Tuneables,
        functions: &FuncsInstance,
    ) -> naga::valid::ModuleInfo {
        // Our own sanity checks
        assert!(
            !module.entry_points.is_empty(),
            "some drivers don't like when a module contains no entry points"
        );

        #[cfg(debug_assertions)]
        let flags = naga::valid::ValidationFlags::all();
        #[cfg(not(debug_assertions))]
        let flags = naga::valid::ValidationFlags::empty();

        let capabilities = if tuneables.hardware_supports_f64 {
            naga::valid::Capabilities::FLOAT64
        } else {
            naga::valid::Capabilities::empty()
        };
        let res = naga::valid::Validator::new(flags, capabilities).validate(&module);

        let info = match res {
            Ok(info) => info,
            Err(e) => Self::validation_panic(e, &module, tuneables, functions, capabilities),
        };

        return info;
    }

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
            let base_function = active_module.declare_base_function(*ptr, function_data);
            base_functions.insert(*ptr, base_function);
            let entry_function = active_module.declare_entry_function(*ptr, function_data);
            entry_functions.insert(*ptr, entry_function);
        }

        // Declare brain function
        let mut brain_function = active_module.declare_brain_function();

        // Declare recursive functions after brain
        for ptr in call_order.get_in_order() {
            let function_data = functions
                .get(*ptr)
                .expect("call order doesn't invent functions");
            let stack_function = active_module.declare_stack_function(*ptr, function_data);
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
            entry_function.populate_entry_function(
                function_data,
                base_handle,
                &base_args,
                &base_res,
            )?;
        }

        // Populate monofunctions
        brain_function.populate(&mut active_module, &stack_functions);

        Self::appease_drivers(&mut module);

        Ok(Self {
            module_info: Self::validate(&module, tuneables, functions),
            module,
        })
    }
}
