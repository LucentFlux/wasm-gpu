use std::{collections::HashMap, sync::atomic::AtomicUsize};

use itertools::Itertools;
use wasm_opcodes::{ControlFlowOperator, OperatorByProposal};
use wasmparser::ValType;
use wasmtime_environ::Trap;

use crate::{
    build,
    module_ext::{BlockExt, ExpressionsExt},
    std_objects::StdObjects,
    BuildError, FuncAccessible, FunctionModuleData,
};

use super::{
    locals::{FnLocal, FnLocals},
    results::WasmFnResTy,
};

mod mvp;

/// Blocks have parameters that they take off the stack, and results that they put back
#[derive(Clone, Debug)]
pub(super) struct BlockType {
    pub arguments: Vec<ValType>,
    pub results: Vec<ValType>,
}
impl BlockType {
    pub(super) fn from_parsed(
        parsed: wasmparser::BlockType,
        func_types: HashMap<u32, wasmparser::FuncType>,
    ) -> Self {
        match parsed {
            wasmparser::BlockType::Empty => Self {
                arguments: Vec::new(),
                results: Vec::new(),
            },
            wasmparser::BlockType::Type(t) => Self {
                arguments: Vec::new(),
                results: vec![t],
            },
            wasmparser::BlockType::FuncType(ft_id) => {
                let ty = func_types
                    .get(&ft_id)
                    .expect("block type referred to function type id that wasn't present in map");
                Self {
                    arguments: Vec::from(ty.params()),
                    results: Vec::from(ty.results()),
                }
            }
        }
    }

    /// Used to find the 'type' of the outermost 'block' of a function body
    pub(super) fn from_return_type(return_type: Option<Vec<ValType>>) -> Self {
        Self {
            arguments: Vec::new(),
            results: return_type.unwrap_or_default(),
        }
    }
}

/// The webassembly `br` instruction has the ability to pierce through multiple layers of blocks at once.
/// To track this in our shader code, we assign an 'is_branching' boolean at each block layer, which
/// is used to check (on exit from a child block) whether the child is requesting that the branch continues
/// down the chain of blocks.
///
/// This is excessive, and we could optimise this system to only include propogation variables where required,
/// but this reduces the simplicity of our code and may introduce bugs. Instead, we trust the optimising compiler
/// of both spirv-tools and the driver to remove excess, leaving us to focus on correctness.
#[derive(Clone)]
pub(crate) struct BlockLabel {
    is_branching: FnLocal,
}
impl BlockLabel {
    fn new(
        block_id: usize,
        local_variables: &mut naga::Arena<naga::LocalVariable>,
        expressions: &mut naga::Arena<naga::Expression>,
        std_objects: &StdObjects,
    ) -> Self {
        Self {
            is_branching: FnLocal::append_to(
                format!("branching_escape_flag_{}", block_id),
                local_variables,
                expressions,
                std_objects.bool.ty,
                Some(std_objects.bool.const_false),
            ),
        }
    }
}

/// Data shared across an entire function body, shared by many blocks and mutable references are built into as blocks are populated.
pub(crate) struct BodyData<'a> {
    // Taken from the definition of the function
    accessible: &'a FuncAccessible,
    module_data: &'a FunctionModuleData,
    return_type: &'a Option<WasmFnResTy>,
    locals: &'a FnLocals,

    // Pulled from the module and function
    constants: &'a mut naga::Arena<naga::Constant>,
    expressions: &'a mut naga::Arena<naga::Expression>,
    local_variables: &'a mut naga::Arena<naga::LocalVariable>,
    std_objects: &'a StdObjects,

    /// To allow reuse of labels across blocks, we first pop from this, then create if that gave None
    unused_labels: Vec<BlockLabel>,

    /// Used to generate unique IDs for each block
    block_count: AtomicUsize,
}

impl<'a> BodyData<'a> {
    pub(crate) fn new(
        accessible: &'a FuncAccessible,
        module_data: &'a FunctionModuleData,
        return_type: &'a Option<WasmFnResTy>,
        locals: &'a FnLocals,
        constants: &'a mut naga::Arena<naga::Constant>,
        expressions: &'a mut naga::Arena<naga::Expression>,
        local_variables: &'a mut naga::Arena<naga::LocalVariable>,
        std_objects: &'a StdObjects,
    ) -> Self {
        Self {
            accessible,
            module_data,
            return_type,
            locals,
            constants,
            expressions,
            local_variables,
            std_objects,
            unused_labels: vec![],
            block_count: AtomicUsize::new(0),
        }
    }

    /// Return results from the current stack, or just returns if the function has no such return values.
    fn push_return(&mut self, block: &mut naga::Block, stack: Vec<naga::Handle<naga::Expression>>) {
        if let Some(return_type) = &self.return_type {
            let required_values = return_type.components().len();
            assert!(stack.len() >= required_values);
            let results = stack[stack.len() - required_values..].to_vec();
            return_type.push_return(self.expressions, block, results);
        } else {
            block.push_empty_return()
        }
    }

    /// Used in the outer function scope once the final block has been completed to emit the final return
    pub(crate) fn push_final_return(&mut self, block: &mut naga::Block, results: Vec<FnLocal>) {
        let mut result_expressions = Vec::new();

        for local in results {
            // Load values as expressions
            let ptr = local.expression;
            let expression = self.expressions.append_load(ptr);
            block.push_emit(expression);
            result_expressions.push(expression)
        }
        self.push_return(block, result_expressions);
    }
}

/// When we're building the body of a function, we don't actually need to know what the function is.
/// We can simply build the block objects, and keep a mutable reference to the expressions within the
/// function, and go from there.
pub(crate) struct ActiveBlock<'a> {
    body_data: BodyData<'a>,

    parents: Vec<BlockLabel>,
    own_label: BlockLabel,

    /// Blocks take arguments off of the stack and push back results, like inline functions. To manage these
    /// values in a control-flow independent way (e.g. for looping or branching blocks) we take the parameters
    /// as locals and pass them back as locals to be read.
    arguments: Vec<FnLocal>,
    results: Vec<FnLocal>,

    block: naga::Block,

    stack: Vec<naga::Handle<naga::Expression>>,
}

impl<'a> ActiveBlock<'a> {
    pub(super) fn new(
        block_type: BlockType,
        mut body_data: BodyData<'a>,
        parents: Vec<BlockLabel>,
    ) -> Self {
        let block_id = body_data
            .block_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);

        let arguments = FnLocal::append_wasm_set_to(
            format!("block_{}_arguments", block_id),
            &mut body_data.local_variables,
            &mut body_data.expressions,
            &body_data.std_objects,
            block_type.arguments,
        );
        let results = FnLocal::append_wasm_set_to(
            format!("block_{}_results", block_id),
            &mut body_data.local_variables,
            &mut body_data.expressions,
            &body_data.std_objects,
            block_type.results,
        );

        let own_label = body_data.unused_labels.pop().unwrap_or_else(|| {
            BlockLabel::new(
                block_id,
                &mut body_data.local_variables,
                &mut body_data.expressions,
                &body_data.std_objects,
            )
        });

        let mut block = naga::Block::new();
        // Clear is_branching flag
        block.push_store(
            own_label.is_branching.expression,
            body_data
                .expressions
                .append_constant(body_data.std_objects.bool.const_false),
        );

        // First thing we do is load arguments into stack
        let mut stack = Vec::new();
        for argument in &arguments {
            let load_expression = body_data.expressions.append_load(argument.expression);
            block.push_emit(load_expression);
            stack.push(load_expression);
        }

        Self {
            body_data,
            parents,
            own_label,
            arguments,
            results,
            block,
            stack,
        }
    }

    pub(crate) fn assign_arguments(
        &self,
        block: &mut naga::Block,
        values: Vec<naga::Handle<naga::Expression>>,
    ) {
        for (value, local) in values.into_iter().zip_eq(&self.arguments) {
            block.push_store(local.expression, value);
        }
    }

    /// Populates a non-looping (straight) block, and hands back the resulting block, as well as
    /// the resulting stack from this block in the form of local variables that can be read
    pub(crate) fn populate_straight(
        mut self,
        instructions: &mut impl Iterator<Item = OperatorByProposal>,
    ) -> build::Result<Self> {
        loop {
            let operation = self.read_basic_block(instructions)?;
            match operation {
                ControlFlowOperator::End => break,
                ControlFlowOperator::Block { blockty } => todo!(),
                ControlFlowOperator::Loop { blockty } => todo!(),
                ControlFlowOperator::If { blockty } => todo!(),
                ControlFlowOperator::Else => todo!(),
                ControlFlowOperator::Br { relative_depth } => todo!(),
                ControlFlowOperator::BrIf { relative_depth } => todo!(),
                ControlFlowOperator::BrTable {
                    targets,
                    default_target,
                } => todo!(),
                ControlFlowOperator::Return => todo!(),
                ControlFlowOperator::Call { function_index } => todo!(),
                ControlFlowOperator::CallIndirect {
                    type_index,
                    table_index,
                    table_byte,
                } => todo!(),
            }
        }

        Ok(self)
    }

    pub(crate) fn finish(mut self) -> (naga::Block, Vec<FnLocal>, BodyData<'a>) {
        // Write back stack
        for (value, local) in self.stack.into_iter().zip_eq(&self.results) {
            self.block.push_store(local.expression, value);
        }

        // Put label back
        self.body_data.unused_labels.push(self.own_label);

        // Deconstruct
        return (self.block, self.results, self.body_data);
    }

    /// Fills instructions until some control flow instruction
    fn read_basic_block(
        &mut self,
        instructions: &mut impl Iterator<Item = OperatorByProposal>,
    ) -> build::Result<ControlFlowOperator> {
        let mut instructions = instructions.peekable();
        let mut last_op = None;
        while let Some(operation) = instructions.peek() {
            match operation {
                OperatorByProposal::ControlFlow(found_last_op) => {
                    last_op = Some(found_last_op.clone());
                    break;
                }
                OperatorByProposal::MVP(mvp_op) => mvp::eat_mvp_operator(self, mvp_op)?,
                OperatorByProposal::Exceptions(_)
                | OperatorByProposal::TailCall(_)
                | OperatorByProposal::ReferenceTypes(_)
                | OperatorByProposal::SignExtension(_)
                | OperatorByProposal::SaturatingFloatToInt(_)
                | OperatorByProposal::BulkMemory(_)
                | OperatorByProposal::Threads(_)
                | OperatorByProposal::SIMD(_)
                | OperatorByProposal::RelaxedSIMD(_) => {
                    return Err(BuildError::UnsupportedInstructionError {
                        instruction_opcode: operation.opcode(),
                    })
                }
            };

            // If it wasn't control flow, actually progress iterator since we implemented the operation
            instructions.next();
        }

        return Ok(last_op
            .expect("due to validation, every block has a control flow instruction at the end"));
    }
}

// Methods used by instruction implementations to modify a working block
impl<'a> ActiveBlock<'a> {
    /// Pushes an expression on to the current stack
    fn push(&mut self, value: naga::Expression) -> naga::Handle<naga::Expression> {
        let needs_emitting = match &value {
            naga::Expression::FunctionArgument(_)
            | naga::Expression::GlobalVariable(_)
            | naga::Expression::LocalVariable(_)
            | naga::Expression::Constant(_)
            | naga::Expression::CallResult(_)
            | naga::Expression::AtomicResult { .. } => false,
            _ => true,
        };

        let handle = self
            .body_data
            .expressions
            .append(value, naga::Span::UNDEFINED);
        self.stack.push(handle);

        if needs_emitting {
            self.append(naga::Statement::Emit(naga::Range::new_from_bounds(
                handle, handle,
            )));
        }

        return handle;
    }

    /// Pops an expression from the current stack
    fn pop(&mut self) -> naga::Handle<naga::Expression> {
        self.stack
            .pop()
            .expect("wasm validation asserts local stack will not be empty")
    }

    /// Peeks an expression from the current stack, for operations like local.tee
    fn peek(&self) -> naga::Handle<naga::Expression> {
        self.stack
            .last()
            .expect("wasm validation asserts local stack will not be empty")
            .clone()
    }

    /// Appends a statement to the current block
    fn append(&mut self, statement: naga::Statement) {
        self.block.push(statement, naga::Span::UNDEFINED);
    }

    /// Calls trap, recording the given flag
    fn append_trap(&mut self, trap_id: Trap) -> build::Result<()> {
        let val = self
            .body_data
            .std_objects
            .trap_values
            .get(&Some(trap_id))
            .expect("unknown trap id")
            .clone();

        let trap_val_handle = self.body_data.expressions.append_constant(val);

        let trap_fn_handle = self.std_objects().trap_fn;

        self.append(naga::Statement::Call {
            function: trap_fn_handle,
            arguments: vec![trap_val_handle],
            result: None,
        });

        Ok(())
    }

    fn push_const_val(&mut self, value: wasm_types::Val) -> build::Result<()> {
        let constant = self
            .body_data
            .std_objects
            .make_wasm_constant(self.body_data.constants, value)?;
        self.push(naga::Expression::Constant(constant));

        Ok(())
    }

    fn local_ptr(&self, local_index: u32) -> naga::Handle<naga::Expression> {
        self.body_data.locals.get(local_index).expression
    }

    fn std_objects(&self) -> &StdObjects {
        &self.body_data.std_objects
    }

    /// Calls a function and pushes the result of the call onto the stack
    fn push_call(
        &mut self,
        function: naga::Handle<naga::Function>,
        arguments: Vec<naga::Handle<naga::Expression>>,
    ) -> build::Result<()> {
        let result = self.push(naga::Expression::CallResult(function));

        self.append(naga::Statement::Call {
            function,
            arguments,
            result: Some(result),
        });

        Ok(())
    }

    /// Pops two arguments, then calls a function and pushes the result
    fn pop_two_push_call_bi(
        &mut self,
        function: naga::Handle<naga::Function>,
    ) -> build::Result<()> {
        let rhs = self.pop();
        let lhs = self.pop();
        self.push_call(function, vec![lhs, rhs])
    }
}
