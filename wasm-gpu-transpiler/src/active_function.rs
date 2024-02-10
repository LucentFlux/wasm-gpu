mod active_block;
mod arguments;
mod locals;
mod results;

use crate::typed::FuncRef;
use naga::Handle;
use naga_ext::{naga_expr, BlockContext, BlockExt, ExpressionsExt, ModuleExt};

use self::active_block::{ActiveBlock, BlockType, BodyData};
use self::results::WasmFnResTy;
use self::{
    arguments::{EntryArguments, WasmFnArgs},
    locals::FnLocals,
};

use crate::active_function::active_block::EndInstruction;
use crate::{build, get_entry_name, std_objects::StdObjects, FuncUnit};

use crate::active_module::ActiveModule;

/// A set of handles to a function that can be 'activated' given a mutable reference to a module
pub(crate) trait InactiveFunction {
    type Active<'f, 'm: 'f>: ActiveFunction<'f, 'm>
    where
        Self: 'f;

    fn activate<'f, 'm: 'f>(&'f self, module: &'f mut ActiveModule<'m>) -> Self::Active<'f, 'm>;
}

/// Any function, entry or not, that can be modified.
pub(crate) trait ActiveFunction<'f, 'm: 'f>:
    'f + Into<BlockContext<'f>> + From<&'f mut Self>
{
    fn fn_mut<'b>(&'b mut self) -> &'b mut naga::Function
    where
        'f: 'b;
    fn std_objects<'b>(&'b self) -> &'b StdObjects
    where
        'f: 'b;
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
            function,
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
    pub(crate) fn populate_base_function(&mut self, func_data: &FuncUnit) -> build::Result<()> {
        let Self {
            working_module,
            data,
        } = self;

        // Decompose this into the parts needed for the blocks
        let accessible = &func_data.accessible;
        let module_data = func_data.data.module_data.as_ref();
        let return_type = &data.wasm_results;
        let block_type =
            BlockType::from_return_type(return_type.as_ref().map(|ty| ty.components().clone()));
        let ActiveModule {
            module,
            std_objects,
            ..
        } = working_module;
        let types = &mut module.types;
        let const_expressions = &mut module.const_expressions;
        let constants = &mut module.constants;
        let function = module.functions.get_mut(data.handle);
        let mut body_data = BodyData::new(
            accessible,
            module_data,
            return_type,
            &data.locals,
            types,
            const_expressions,
            constants,
            &mut function.expressions,
            &mut function.local_variables,
            &std_objects,
            working_module.uses_disjoint_memory,
        );

        // Define base block
        let base_block = ActiveBlock::new(&mut function.body, block_type, &mut body_data, vec![]);

        // Parse instructions
        let mut instructions = func_data.data.operators.iter().peekable();

        // Populate recursively
        let (base_block, end) = base_block.populate_straight(&mut instructions)?;
        assert_eq!(end, EndInstruction::End);
        let (results, control_flow_state) = base_block.finish();

        debug_assert!(instructions.next().is_none(), "validation ensures that all instructions are within the body and that blocks are balanced");

        // Return results if there's a chance control flow exits the end of the block of the body
        if control_flow_state.upper_unconditional_depth.is_none() {
            body_data.push_final_return(&mut function.body, results)
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

impl<'f, 'm: 'f> From<ActiveInternalFunction<'f, 'm>> for BlockContext<'f> {
    fn from(value: ActiveInternalFunction<'f, 'm>) -> Self {
        let ActiveModule { module, .. } = value.working_module;
        let func = module.functions.get_mut(value.data.handle);

        BlockContext {
            types: &mut module.types,
            constants: &mut module.constants,
            const_expressions: &mut module.const_expressions,
            expressions: &mut func.expressions,
            locals: &mut func.local_variables,
            block: &mut func.body,
        }
    }
}

impl<'f, 'm: 'f> From<&'f mut ActiveInternalFunction<'f, 'm>> for ActiveInternalFunction<'f, 'm> {
    fn from(value: &'f mut ActiveInternalFunction<'f, 'm>) -> Self {
        Self {
            working_module: value.working_module,
            data: value.data,
        }
    }
}

impl<'f, 'm: 'f> ActiveFunction<'f, 'm> for ActiveInternalFunction<'f, 'm> {
    fn fn_mut<'b>(&'b mut self) -> &'b mut naga::Function
    where
        'f: 'b,
    {
        self.working_module
            .module
            .functions
            .get_mut(self.data.handle)
    }
    fn std_objects<'b>(&'b self) -> &'b StdObjects
    where
        'f: 'b,
    {
        &self.working_module.std_objects
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
    ) -> Self {
        let name = get_entry_name(ptr);

        let mut function = naga::Function::default();

        let args = EntryArguments::append_to(&mut function, std_objects);

        let index = module.entry_points.len();
        module.entry_points.push(naga::EntryPoint {
            name,
            stage: naga::ShaderStage::Compute,
            early_depth_test: None,
            workgroup_size: [crate::WORKGROUP_SIZE, 1, 1],
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
        let invocation_id = self.get_workgroup_index();

        let constants_buffer = self.std_objects().preamble.bindings.constants;
        let constants_buffer = self.fn_mut().expressions.append_global(constants_buffer);
        let invocations_count = naga_expr!(self => Load(constants_buffer[const crate::TOTAL_INVOCATIONS_CONSTANT_INDEX]));

        // Write entry globals
        let instance_id_global = self.std_objects().preamble.instance_id;
        let invocation_id_ptr = naga_expr!(self => Global(instance_id_global));
        self.fn_mut()
            .body
            .push_store(invocation_id_ptr, invocation_id);

        let invocations_count_global = self.std_objects().preamble.invocations_count;
        let invocations_count_ptr = naga_expr!(self => Global(invocations_count_global));
        self.fn_mut()
            .body
            .push_store(invocations_count_ptr, invocations_count);

        // Don't execute if we're beyond the invocation count
        let is_not_being_executed = naga_expr!(self => invocation_id > invocations_count);
        let mut if_not_being_executed = naga::Block::default();
        if_not_being_executed.push_bare_return();
        self.fn_mut().body.push_if(
            is_not_being_executed,
            if_not_being_executed,
            naga::Block::default(),
        );

        // Call fn
        let arguments = self.read_entry_inputs(arguments, invocation_id);
        let results: Option<(&WasmFnResTy, Handle<naga::Expression>)> =
            results_ty.as_ref().map(|results_ty| {
                (
                    results_ty,
                    self.fn_mut().expressions.append(
                        naga::Expression::CallResult(base_function),
                        naga::Span::UNDEFINED,
                    ),
                )
            });
        self.fn_mut().body.push(
            naga::Statement::Call {
                function: base_function,
                arguments,
                result: results.map(|v| v.1),
            },
            naga::Span::UNDEFINED,
        );

        // Write outputs
        if let Some((results_ty, results_expr)) = results {
            self.store_output(results_ty, invocation_id, results_expr)?;
        }

        // Write trap status
        let flag_state = self.working_module.std_objects.preamble.trap_state;
        let flags_buffer = self.working_module.std_objects.preamble.bindings.flags;
        let flag_state = naga_expr!(self => Load(Global(flag_state)));
        let write_word_loc =
            naga_expr!(self => Global(flags_buffer)[invocation_id][const crate::TRAP_FLAG_INDEX]);
        self.fn_mut().body.push_store(write_word_loc, flag_state);

        return Ok(());
    }
}

impl<'f, 'm: 'f> From<ActiveEntryFunction<'f, 'm>> for BlockContext<'f> {
    fn from(value: ActiveEntryFunction<'f, 'm>) -> Self {
        let func = &mut value
            .working_module
            .module
            .entry_points
            .get_mut(value.data.index)
            .expect("entry points cannot be removed")
            .function;

        BlockContext {
            types: &mut value.working_module.module.types,
            constants: &mut value.working_module.module.constants,
            const_expressions: &mut value.working_module.module.const_expressions,
            expressions: &mut func.expressions,
            locals: &mut func.local_variables,
            block: &mut func.body,
        }
    }
}

impl<'f, 'm: 'f> From<&'f mut ActiveEntryFunction<'f, 'm>> for ActiveEntryFunction<'f, 'm> {
    fn from(value: &'f mut ActiveEntryFunction<'f, 'm>) -> Self {
        Self {
            working_module: value.working_module,
            data: value.data,
        }
    }
}

impl<'f, 'm: 'f> ActiveFunction<'f, 'm> for ActiveEntryFunction<'f, 'm> {
    fn fn_mut<'b>(&'b mut self) -> &'b mut naga::Function
    where
        'f: 'b,
    {
        let module = &mut self.working_module.module;
        &mut module
            .entry_points
            .get_mut(self.data.index)
            .expect("entry points are add only")
            .function
    }
    fn std_objects<'b>(&'b self) -> &'b StdObjects
    where
        'f: 'b,
    {
        &self.working_module.std_objects
    }
}
