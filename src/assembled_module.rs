use wasmparser::ValType;

use crate::instance::func::FuncsInstance;
use crate::module::operation::OpCode;
use crate::{Tuneables, Val};

use super::bindings_gen::BindingHandles;
use super::brain_func_gen::populate_brain_func;
use super::call_graph::{CallGraph, CallOrder};
use super::func_gen::{
    populate_base_function, populate_entry_function, WorkingBaseFunction, WorkingEntryFunction,
};
use super::function_collection::FunctionCollection;
use super::std_objects::StdObjects;

#[derive(thiserror::Error, Debug, Clone)]
pub enum BuildError {
    #[error("wasm contained an unsupported instruction")]
    UnsupportedInstructionError { instruction_opcode: OpCode },
    #[error("wasm contained an unsupported type")]
    UnsupportedTypeError { wasm_type: ValType },
}

pub mod build {
    use super::BuildError;

    pub type Result<V> = std::result::Result<V, BuildError>;
}

// The values used when building a module
pub struct WorkingModule<'a> {
    pub module: &'a mut naga::Module,
    pub std_objs: &'a StdObjects,
    pub tuneables: &'a Tuneables,
    pub base_functions: FunctionCollection,
    pub stack_functions: FunctionCollection,
    pub brain_function: naga::Handle<naga::Function>,
    pub call_order: CallOrder,
}

impl<'a> WorkingModule<'a> {
    fn new(
        module: &'a mut naga::Module,
        std_objs: &'a StdObjects,
        tuneables: &'a Tuneables,
        functions: &FuncsInstance,
    ) -> Self {
        // Calculate direct call graph to figure out if two functions are directly corecursive
        let call_graph = CallGraph::calculate(functions);
        let call_order = call_graph.to_call_order();

        // Generate function objects
        let base_functions = FunctionCollection::new(&mut module.functions, &call_order);
        let stack_functions = FunctionCollection::new(&mut module.functions, &call_order);
        let brain_function = module
            .functions
            .append(naga::Function::default(), naga::Span::UNDEFINED);

        Self {
            module,
            std_objs,
            tuneables,
            base_functions,
            stack_functions,
            brain_function,
            call_order,
        }
    }

    /// Get's a WASM val type's naga type
    pub fn get_val_type(&mut self, val_ty: ValType) -> build::Result<naga::Handle<naga::Type>> {
        match val_ty {
            ValType::I32 => self.std_objs.tys.wasm_i32.get(self),
            ValType::I64 => self.std_objs.tys.wasm_i64.get(self),
            ValType::F32 => self.std_objs.tys.wasm_f32.get(self),
            ValType::F64 => self.std_objs.tys.wasm_f64.get(self),
            ValType::V128 => self.std_objs.tys.wasm_v128.get(self),
            ValType::FuncRef => self.std_objs.tys.wasm_func_ref.get(self),
            ValType::ExternRef => self.std_objs.tys.wasm_extern_ref.get(self),
        }
    }

    /// Makes a new constant from the value
    pub fn constant(&mut self, value: Val) -> build::Result<naga::Handle<naga::Constant>> {
        match value {
            Val::I32(value) => self.std_objs.tys.wasm_i32.make_const(self, value),
            Val::I64(value) => self.std_objs.tys.wasm_i64.make_const(self, value),
            Val::F32(value) => self.std_objs.tys.wasm_f32.make_const(self, value),
            Val::F64(value) => self.std_objs.tys.wasm_f64.make_const(self, value),
            Val::V128(value) => self.std_objs.tys.wasm_v128.make_const(self, value),
            Val::FuncRef(value) => self.std_objs.tys.wasm_func_ref.make_const(self, value),
            Val::ExternRef(value) => self.std_objs.tys.wasm_extern_ref.make_const(self, value),
        }
    }

    pub fn make_function<'b>(
        &'b mut self,
    ) -> (WorkingBaseFunction<'a, 'b>, naga::Handle<naga::Function>) {
        let func = naga::Function::default();
        let handle = self.module.functions.append(func, naga::Span::UNDEFINED);
        let working = WorkingBaseFunction::new(self, handle.clone());
        (working, handle)
    }

    fn make_entry_function<'b>(&'b mut self, name: String) -> WorkingEntryFunction<'a, 'b> {
        let func = naga::Function::default();
        let index = self.module.entry_points.len();
        self.module.entry_points.push(naga::EntryPoint {
            name,
            stage: naga::ShaderStage::Compute,
            early_depth_test: None,
            workgroup_size: self.tuneables.workgroup_size,
            function: func,
        });
        WorkingEntryFunction::new(self, index)
    }
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
    ) -> build::Result<Self> {
        let mut module = naga::Module::default();
        let objects = StdObjects::new();
        let mut working = WorkingModule::new(&mut module, &objects, tuneables, &functions);

        // Generate bindings used for all wasm things like globals
        let bindings = BindingHandles::new(&mut working)?;

        // Populate function bodies
        for ptr in functions.all_ptrs() {
            let function_data = functions.get(&ptr);

            // Generate function bodies
            let base_handle = working.base_functions.lookup(&ptr.to_func_ref());
            let mut working_function = WorkingBaseFunction::new(&mut working, base_handle);
            let (arg_tys, ret_ty) =
                populate_base_function(&mut working_function, function_data, &bindings)?;
            //populate_stack_function(&mut module, function_data, &call_order, stack_functions.lookup(&ptr.to_func_ref()))?;

            // Generate entry function that points into base
            let mut entry_function = working.make_entry_function(ptr.get_entry_name());
            populate_entry_function(
                &mut entry_function,
                ptr,
                base_handle,
                function_data,
                &bindings,
                arg_tys,
                ret_ty,
            )?;
        }

        // Generate brain function
        populate_brain_func(&mut working)?;

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
