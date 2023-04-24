mod active_basic_block;
mod arguments;
mod block_gen;
mod body_gen;
mod locals;
mod mvp;
mod results;

use naga::Handle;
use wasm_opcodes::OperatorByProposal;
use wasm_types::FuncRef;

use crate::module_ext::{BlockExt, FunctionExt};
use crate::{
    build, get_entry_name, module_ext::ModuleExt, naga_expr, std_objects::StdObjects, FuncUnit,
};

use self::results::WasmFnResTy;
use self::{
    arguments::{EntryArguments, WasmFnArgs},
    locals::FnLocals,
};
use self::{block_gen::populate_block, body_gen::FunctionBodyInformation};

use crate::active_module::ActiveModule;

/// A set of handles to a function that can be 'activated' given a mutable reference to a module
pub(crate) trait InactiveFunction {
    type Active<'f, 'm: 'f>: ActiveFunction<'f, 'm>
    where
        Self: 'f;

    fn activate<'f, 'm: 'f>(&'f self, module: &'f mut ActiveModule<'m>) -> Self::Active<'f, 'm>;
}

/// Any function, entry or not, that can be modified.
pub(crate) trait ActiveFunction<'f, 'm: 'f> {
    fn get_active<'b>(&'b mut self) -> (MutModuleWithoutFunctions<'b>, &'b mut naga::Function)
    where
        'f: 'b;
    fn get_module_mut<'b>(&'b mut self) -> &'b mut ActiveModule<'m>
    where
        'f: 'b;
    fn get_module<'b>(&'b self) -> &'b ActiveModule<'m>
    where
        'f: 'b;

    fn get_mut<'b>(&'b mut self) -> &'b mut naga::Function
    where
        'f: 'b,
    {
        self.get_active().1
    }
    fn std_objects<'b>(&'b self) -> &'b StdObjects
    where
        'f: 'b,
    {
        &self.get_module().std_objs
    }
    fn make_wasm_constant(
        &mut self,
        value: wasm_types::Val,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        self.get_module_mut().make_wasm_constant(value)
    }
}

/// While working on a function we may wish to modify the module the function is in.
/// This contains references to everything *other* than the functions in a module, to
/// allow mutable references to both to coexist.
pub(crate) struct MutModuleWithoutFunctions<'a> {
    pub(crate) types: &'a mut naga::UniqueArena<naga::Type>,
    pub(crate) constants: &'a mut naga::Arena<naga::Constant>,
    pub(crate) global_variables: &'a mut naga::Arena<naga::GlobalVariable>,
}

pub(crate) struct InternalFunction {
    handle: Handle<naga::Function>,
    wasm_arguments: WasmFnArgs,
    wasm_results: Option<WasmFnResTy>,
    locals: FnLocals,
}

impl InternalFunction {
    pub(crate) fn append_declaration_to(
        module: &mut naga::Module,
        std_objects: &StdObjects,
        name_suffix: &str,
        ptr: FuncRef,
        function_definition: &FuncUnit,
    ) -> build::Result<Self> {
        let name = get_entry_name(ptr) + name_suffix;

        let wasm_results =
            WasmFnResTy::make_type(module, std_objects, function_definition.data.ty.results());

        let handle = module.new_empty_function(name);
        let function = module.fn_mut(handle);

        let wasm_arguments =
            WasmFnArgs::append_to(function, std_objects, function_definition.data.ty.params());
        if let Some(wasm_results) = &wasm_results {
            wasm_results.set_return_type(function)
        }

        let locals = FnLocals::append_to(
            module,
            handle,
            std_objects,
            &function_definition.data.locals,
            &wasm_arguments,
        )?;

        Ok(Self {
            handle,
            wasm_arguments,
            wasm_results,
            locals,
        })
    }
}

impl InactiveFunction for InternalFunction {
    type Active<'f, 'm: 'f> = ActiveInternalFunction<'f, 'm>;

    fn activate<'f, 'm: 'f>(
        &'f self,
        working_module: &'f mut ActiveModule<'m>,
    ) -> Self::Active<'f, 'm> {
        ActiveInternalFunction {
            working_module,
            data: self,
        }
    }
}

pub(crate) struct ActiveInternalFunction<'f, 'm: 'f> {
    working_module: &'f mut ActiveModule<'m>,
    data: &'f InternalFunction,
}

impl<'f, 'm: 'f> ActiveInternalFunction<'f, 'm> {
    pub(crate) fn handle(&self) -> naga::Handle<naga::Function> {
        self.data.handle
    }

    /// Populates the body of a base function that doesn't use the stack.
    pub(crate) fn populate_base_function(&mut self, function: &FuncUnit) -> build::Result<()> {
        // Parse instructions
        let accessible = &function.accessible;
        let module_data = function.data.module_data.as_ref();
        let mut instructions = function
            .data
            .operators
            .iter()
            .map(OperatorByProposal::clone);
        let body_info = FunctionBodyInformation {
            accessible,
            module_data,
        };
        let entry_stack = vec![];
        let exit_stack = populate_block(self, &mut instructions, entry_stack, body_info)?;

        // Return results
        if let Some(result_type) = &self.data.wasm_results {
            result_type.push_return(self.get_mut(), exit_stack);
        }

        return Ok(());
    }

    pub(crate) fn get_arg_tys(&self) -> &WasmFnArgs {
        &self.data.wasm_arguments
    }

    pub(crate) fn get_res_ty(&self) -> &Option<WasmFnResTy> {
        &self.data.wasm_results
    }
}

impl<'f, 'm: 'f> ActiveFunction<'f, 'm> for ActiveInternalFunction<'f, 'm> {
    fn get_active<'b>(&'b mut self) -> (MutModuleWithoutFunctions<'b>, &'b mut naga::Function)
    where
        'f: 'b,
    {
        let module = &mut self.working_module.module;
        (
            MutModuleWithoutFunctions {
                types: &mut module.types,
                constants: &mut module.constants,
                global_variables: &mut module.global_variables,
            },
            module.functions.get_mut(self.data.handle),
        )
    }

    fn get_module<'b>(&'b self) -> &'b ActiveModule<'m>
    where
        'f: 'b,
    {
        self.working_module
    }

    fn get_module_mut<'b>(&'b mut self) -> &'b mut ActiveModule<'m>
    where
        'f: 'b,
    {
        self.working_module
    }
}

pub(crate) struct EntryFunction {
    index: usize,
    args: EntryArguments,
}

impl EntryFunction {
    pub(crate) fn append_declaration_to(
        module: &mut naga::Module,
        std_objects: &StdObjects,
        ptr: FuncRef,
        workgroup_size: u32,
    ) -> Self {
        let name = get_entry_name(ptr);

        let mut function = naga::Function::default();

        let args = EntryArguments::append_to(&mut function, std_objects);

        let index = module.entry_points.len();
        module.entry_points.push(naga::EntryPoint {
            name,
            stage: naga::ShaderStage::Compute,
            early_depth_test: None,
            workgroup_size: [workgroup_size, 1, 1],
            function,
        });

        Self { index, args }
    }
}

impl InactiveFunction for EntryFunction {
    type Active<'f, 'm: 'f> = ActiveEntryFunction<'f, 'm>;

    fn activate<'f, 'm: 'f>(
        &'f self,
        working_module: &'f mut ActiveModule<'m>,
    ) -> Self::Active<'f, 'm> {
        ActiveEntryFunction {
            working_module,
            data: self,
        }
    }
}

pub(crate) struct ActiveEntryFunction<'f, 'm: 'f> {
    working_module: &'f mut ActiveModule<'m>,
    data: &'f EntryFunction,
}

impl<'f, 'm: 'f> ActiveEntryFunction<'f, 'm> {
    fn get_workgroup_index(&mut self) -> naga::Handle<naga::Expression> {
        let global_id = self.data.args.global_id.expression();
        naga_expr! {self => global_id[const 0]}
    }

    fn io_base_index(
        &mut self,
        io_word_alignment: u32,
        instance_index: naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression> {
        naga_expr! {self => U32(io_word_alignment) * instance_index}
    }

    fn read_entry_inputs(
        &mut self,
        arguments: &WasmFnArgs,
        instance_index: naga::Handle<naga::Expression>,
    ) -> Vec<naga::Handle<naga::Expression>> {
        let base_index = self.io_base_index(arguments.word_alignment(), instance_index);

        return arguments.append_read_at(self, base_index);
    }

    fn store_output(
        &mut self,
        ty: &WasmFnResTy,
        instance_index: naga::Handle<naga::Expression>,
        value: naga::Handle<naga::Expression>,
    ) -> build::Result<()> {
        let base_index = self.io_base_index(ty.word_alignment(), instance_index);

        ty.append_store_at(self, base_index, value)
    }

    /// Generates function that extracts arguments from buffer, calls base function,
    /// then writes results to output buffer
    pub(crate) fn populate_entry_function(
        &mut self,
        base_function: naga::Handle<naga::Function>,
        arguments: &WasmFnArgs,
        results_ty: &Option<WasmFnResTy>,
    ) -> build::Result<()> {
        let instance_index = self.get_workgroup_index();

        // Write entry globals
        let invocation_id = self.std_objects().instance_id;
        let invocation_id_ptr = self.get_mut().append_global(invocation_id);
        self.get_mut()
            .body
            .push_store(invocation_id_ptr, instance_index);

        // Call fn
        let arguments = self.read_entry_inputs(arguments, instance_index);
        let results: Option<(&WasmFnResTy, Handle<naga::Expression>)> =
            results_ty.as_ref().map(|results_ty| {
                (
                    results_ty,
                    self.get_mut().expressions.append(
                        naga::Expression::CallResult(base_function),
                        naga::Span::UNDEFINED,
                    ),
                )
            });
        self.get_mut().body.push(
            naga::Statement::Call {
                function: base_function,
                arguments,
                result: results.map(|v| v.1),
            },
            naga::Span::UNDEFINED,
        );

        // Write outputs
        if let Some((results_ty, results_expr)) = results {
            self.store_output(results_ty, instance_index, results_expr)?;
        }

        return Ok(());
    }
}

impl<'f, 'm: 'f> ActiveFunction<'f, 'm> for ActiveEntryFunction<'f, 'm> {
    fn get_active<'b>(&'b mut self) -> (MutModuleWithoutFunctions<'b>, &'b mut naga::Function)
    where
        'f: 'b,
    {
        let module = &mut self.working_module.module;
        (
            MutModuleWithoutFunctions {
                types: &mut module.types,
                constants: &mut module.constants,
                global_variables: &mut module.global_variables,
            },
            &mut module
                .entry_points
                .get_mut(self.data.index)
                .expect("entry points are add only")
                .function,
        )
    }

    fn get_module<'b>(&'b self) -> &'b ActiveModule<'m>
    where
        'f: 'b,
    {
        self.working_module
    }

    fn get_module_mut<'b>(&'b mut self) -> &'b mut ActiveModule<'m>
    where
        'f: 'b,
    {
        self.working_module
    }
}
