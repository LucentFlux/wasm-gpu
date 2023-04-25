mod active_block;
mod arguments;
mod locals;
mod results;

use itertools::Itertools;
use naga::Handle;
use naga_ext::{naga_expr, BlockExt, ModuleExt, ShaderPart};
use wasm_opcodes::OperatorByProposal;
use wasm_types::FuncRef;

use self::active_block::{ActiveBlock, BlockType, BodyData};
use self::results::WasmFnResTy;
use self::{
    arguments::{EntryArguments, WasmFnArgs},
    locals::FnLocals,
};

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
pub(crate) trait ActiveFunction<'f, 'm: 'f>: ShaderPart {
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
        let constants = &mut module.constants;
        let function = module.functions.get_mut(data.handle);
        let body_data = BodyData::new(
            accessible,
            module_data,
            return_type,
            &data.locals,
            constants,
            &mut function.expressions,
            &mut function.local_variables,
            &std_objects,
        );

        // Define base block
        let base_block = ActiveBlock::new(&mut function.body, block_type, body_data, vec![]);

        // Parse instructions
        let mut instructions = func_data
            .data
            .operators
            .iter()
            .map(OperatorByProposal::clone);

        // Populate recursively
        let (results, mut body_data, remaining_parents) =
            base_block.populate_straight(&mut instructions)?.finish();

        debug_assert!(instructions.next().is_none(), "validation ensures that all instructions are within the body and that blocks are balanced");
        debug_assert!(
            remaining_parents.is_empty(),
            "blocks should be balanced and push and pop their own block labels"
        );

        // Return results
        body_data.push_final_return(&mut function.body, results);

        return Ok(());
    }

    pub(crate) fn get_arg_tys(&self) -> &WasmFnArgs {
        &self.data.wasm_arguments
    }

    pub(crate) fn get_res_ty(&self) -> &Option<WasmFnResTy> {
        &self.data.wasm_results
    }
}

impl<'f, 'm: 'f> ShaderPart for ActiveInternalFunction<'f, 'm> {
    fn parts(
        &mut self,
    ) -> (
        &mut naga::Arena<naga::Constant>,
        &mut naga::Arena<naga::Expression>,
        &mut naga::Block,
    ) {
        let ActiveModule { module, .. } = self.working_module;
        let func = module.functions.get_mut(self.data.handle);
        (&mut module.constants, &mut func.expressions, &mut func.body)
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
        let instance_id = self.std_objects().instance_id;
        let invocation_id_ptr = naga_expr!(self => Global(instance_id));
        self.fn_mut()
            .body
            .push_store(invocation_id_ptr, instance_index);

        // Call fn
        let arguments = self.read_entry_inputs(arguments, instance_index);
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
            self.store_output(results_ty, instance_index, results_expr)?;
        }

        return Ok(());
    }
}

impl<'f, 'm: 'f> ShaderPart for ActiveEntryFunction<'f, 'm> {
    fn parts<'b>(
        &mut self,
    ) -> (
        &mut naga::Arena<naga::Constant>,
        &mut naga::Arena<naga::Expression>,
        &mut naga::Block,
    ) {
        let func = &mut self
            .working_module
            .module
            .entry_points
            .get_mut(self.data.index)
            .expect("entry points cannot be removed")
            .function;
        (
            &mut self.working_module.module.constants,
            &mut func.expressions,
            &mut func.body,
        )
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
