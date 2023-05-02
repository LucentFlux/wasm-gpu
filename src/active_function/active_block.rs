use std::{iter::Peekable, sync::atomic::AtomicUsize};

use itertools::Itertools;
use naga_ext::{naga_expr, BlockExt, ExpressionsExt, ShaderPart};
use wasm_opcodes::{ControlFlowOperator, OperatorByProposal};
use wasmparser::ValType;
use wasmtime_environ::Trap;

use crate::{
    build, std_objects::StdObjects, BuildError, ExceededComponent, FuncAccessible,
    FunctionModuleData, MEMORY_STRIDE_WORDS, TOTAL_INVOCATIONS_CONSTANT_INDEX,
};

use super::{
    locals::{FnLocal, FnLocals},
    results::WasmFnResTy,
};

mod mvp;
mod threads;

/// Blocks have parameters that they take off the stack, and results that they put back
#[derive(Clone, Debug)]
pub(super) struct BlockType {
    pub arguments: Vec<ValType>,
    pub results: Vec<ValType>,
}
impl BlockType {
    pub(super) fn from_parsed(
        parsed: wasmparser::BlockType,
        func_types: &Vec<wasmparser::FuncType>,
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
                let ft_id = usize::try_from(ft_id).expect("types must be expressable in memory");
                let ty = func_types
                    .get(ft_id)
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
                std_objects.naga_bool.ty,
                Some(std_objects.naga_bool.const_false),
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

    // Config from tuneables
    uses_disjoint_memory: bool,

    // Standard things we use
    /// A handle to an expression holding a true value
    wasm_true_expression: naga::Handle<naga::Expression>,
    /// A handle to an expression holding a false value
    wasm_false_expression: naga::Handle<naga::Expression>,
    /// A handle to an expression holding a true value
    naga_true_expression: naga::Handle<naga::Expression>,
    /// A handle to an expression holding a false value
    naga_false_expression: naga::Handle<naga::Expression>,

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
        uses_disjoint_memory: bool,
    ) -> Self {
        let wasm_true_expression = expressions.append_constant(std_objects.wasm_bool.const_true);
        let wasm_false_expression = expressions.append_constant(std_objects.wasm_bool.const_false);
        let naga_true_expression = expressions.append_constant(std_objects.naga_bool.const_true);
        let naga_false_expression = expressions.append_constant(std_objects.naga_bool.const_false);

        Self {
            accessible,
            module_data,
            return_type,
            locals,
            constants,
            expressions,
            local_variables,
            std_objects,
            uses_disjoint_memory,
            wasm_true_expression,
            wasm_false_expression,
            naga_true_expression,
            naga_false_expression,
            unused_labels: vec![],
            block_count: AtomicUsize::new(0),
        }
    }

    /// Return results from the current stack, or just returns if the function has no such return values.
    /// Returns the values that were popped from the stack to form the return expression
    fn push_return(
        &mut self,
        block: &mut naga::Block,
        stack: &mut Vec<naga::Handle<naga::Expression>>,
    ) {
        if let Some(return_type) = &self.return_type {
            let required_values = return_type.components().len();
            let mut results = Vec::new();
            for _ in 0..required_values {
                results.push(stack.pop().expect("validation ensures that enough values were on the stack when return is invoked"))
            }
            results.reverse();
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
        self.push_return(block, &mut result_expressions);
    }
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum EndInstruction {
    End,
    Else,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum ControlFlowState {
    /// Denotes that, after this instruction, control flow should continue assuming nothing has changed.
    /// This is used after function calls and assumed as the default for blocks until control flow is
    /// encountered.
    Continue,
    /// Denotes that we are branching within this block to `relative_depth` blocks above this one.
    /// Means that nothing can happen after this instruction, and that there isn't any point checking break
    /// state.
    UnconditionalBranch {
        relative_unconditional_depth: u32,
        relative_additional_conditional_depth: u32,
    },
    /// Denotes that we may be branching within this block or `relative_depth` blocks above this one. Used by `br_if`
    ConditionalBranch { relative_depth: u32 },
}
impl ControlFlowState {
    /// Finds the state given that control flow may have come from the left or righ branch.
    fn union(lhs: ControlFlowState, rhs: ControlFlowState) -> ControlFlowState {
        match (lhs, rhs) {
            // ConditionalBranch { relative_depth } & Continue => ConditionalBranch { relative_depth }
            (
                ControlFlowState::ConditionalBranch { relative_depth },
                ControlFlowState::Continue,
            ) => ControlFlowState::ConditionalBranch { relative_depth },
            (
                ControlFlowState::Continue,
                ControlFlowState::ConditionalBranch { relative_depth },
            ) => ControlFlowState::ConditionalBranch { relative_depth },
            // Continue & Continue => Continue
            (ControlFlowState::Continue, ControlFlowState::Continue) => ControlFlowState::Continue,
            // ConditionalBranch { rd1 } & ConditionalBranch { rd2 } => ConditionalBranch { max(rd1, rd2) }
            (
                ControlFlowState::ConditionalBranch {
                    relative_depth: rd1,
                },
                ControlFlowState::ConditionalBranch {
                    relative_depth: rd2,
                },
            ) => ControlFlowState::ConditionalBranch {
                relative_depth: u32::max(rd1, rd2),
            },
            // UnconditionalBranch { relative_depth } & Continue => ConditionalBranch { relative_depth }
            (
                ControlFlowState::UnconditionalBranch {
                    relative_unconditional_depth,
                    relative_additional_conditional_depth,
                },
                ControlFlowState::Continue,
            ) => ControlFlowState::ConditionalBranch {
                relative_depth: relative_unconditional_depth
                    + relative_additional_conditional_depth,
            },
            (
                ControlFlowState::Continue,
                ControlFlowState::UnconditionalBranch {
                    relative_unconditional_depth,
                    relative_additional_conditional_depth,
                },
            ) => ControlFlowState::ConditionalBranch {
                relative_depth: relative_unconditional_depth
                    + relative_additional_conditional_depth,
            },
            // UnconditionalBranch { rd1 } & ConditionalBranch { rd2 } => ConditionalBranch { max(rd1, rd2) }
            (
                ControlFlowState::UnconditionalBranch {
                    relative_unconditional_depth,
                    relative_additional_conditional_depth,
                },
                ControlFlowState::ConditionalBranch { relative_depth },
            ) => ControlFlowState::ConditionalBranch {
                relative_depth: u32::max(
                    relative_unconditional_depth + relative_additional_conditional_depth,
                    relative_depth,
                ),
            },
            (
                ControlFlowState::ConditionalBranch { relative_depth },
                ControlFlowState::UnconditionalBranch {
                    relative_unconditional_depth,
                    relative_additional_conditional_depth,
                },
            ) => ControlFlowState::ConditionalBranch {
                relative_depth: u32::max(
                    relative_unconditional_depth + relative_additional_conditional_depth,
                    relative_depth,
                ),
            },
            // UnconditionalBranch { uc_rd1, c_rd1 } & UnconditionalBranch { uc_rd2, c_rd2 } => UnconditionalBranch { min(uc_rd1, uc_rd2), max(uc_rd1 + c_rd1, uc_rd2 + c_rd2) - min(uc_rd1, uc_rd2) }
            (
                ControlFlowState::UnconditionalBranch {
                    relative_unconditional_depth: uc_rd1,
                    relative_additional_conditional_depth: c_rd1,
                },
                ControlFlowState::UnconditionalBranch {
                    relative_unconditional_depth: uc_rd2,
                    relative_additional_conditional_depth: c_rd2,
                },
            ) => ControlFlowState::UnconditionalBranch {
                relative_unconditional_depth: u32::min(uc_rd1, uc_rd2),
                relative_additional_conditional_depth: u32::max(uc_rd1 + c_rd1, uc_rd2 + c_rd2)
                    - u32::min(uc_rd1, uc_rd2),
            },
        }
    }

    /// Gives the control flow state of this when viewed from one block above. I.e. an UnconditionalBranch of
    /// depth 1 gives an UnconditionalBranch of depth 0, UnconditionalBranch with depth of 0 and additional
    /// depth of 2 gives a ConditionalBranch with a depth of 2
    fn downgrade(&self) -> ControlFlowState {
        match *self {
            ControlFlowState::Continue
            | ControlFlowState::UnconditionalBranch {
                relative_unconditional_depth: 0,
                relative_additional_conditional_depth: 0,
            }
            | ControlFlowState::ConditionalBranch { relative_depth: 0 } => {
                ControlFlowState::Continue
            }
            ControlFlowState::UnconditionalBranch {
                relative_unconditional_depth: 0,
                relative_additional_conditional_depth,
            } => ControlFlowState::ConditionalBranch {
                relative_depth: relative_additional_conditional_depth,
            },
            ControlFlowState::UnconditionalBranch {
                relative_unconditional_depth,
                relative_additional_conditional_depth,
            } => ControlFlowState::UnconditionalBranch {
                relative_unconditional_depth: relative_unconditional_depth - 1,
                relative_additional_conditional_depth,
            },
            ControlFlowState::ConditionalBranch { relative_depth } => {
                ControlFlowState::ConditionalBranch {
                    relative_depth: relative_depth - 1,
                }
            }
        }
    }
}

/// When we're building the body of a function, we don't actually need to know what the function is.
/// We can simply build the block objects, and keep a mutable reference to the expressions within the
/// function, and go from there.
pub(crate) struct ActiveBlock<'b, 'd> {
    body_data: &'b mut BodyData<'d>,

    parents: Vec<BlockLabel>,

    /// Blocks take arguments off of the stack and push back results, like inline functions. To manage these
    /// values in a control-flow independent way (e.g. for looping or branching blocks) we take the parameters
    /// as locals and pass them back as locals to be read.
    arguments: Vec<FnLocal>,
    results: Vec<FnLocal>,

    /// The block we are populating into. We don't take ownership because we swap this out sometimes, e.g. when
    /// branching in a basic block we begin populating the 'else' block instead.
    ///
    /// Sometimes (e.g. after unconditional branches/returns) we can just throw away instructions.
    /// None represents a dud block that is discarded after the blocks are build. Ultimately this is not
    /// super efficient, but compilers to WASM shouldn't emit instructions that go here so it's
    /// more about matching the specification than being a good engine. (I am a good engine :))
    block: &'b mut naga::Block,

    stack: Vec<naga::Handle<naga::Expression>>,

    exit_state: ControlFlowState,
}

impl<'b, 'd> ActiveBlock<'b, 'd> {
    pub(super) fn new(
        block: &'b mut naga::Block,
        block_type: BlockType,
        body_data: &'b mut BodyData<'d>,
        mut parents: Vec<BlockLabel>,
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

        // Set up block-level data
        let own_label = body_data.unused_labels.pop().unwrap_or_else(|| {
            BlockLabel::new(
                block_id,
                &mut body_data.local_variables,
                &mut body_data.expressions,
                &body_data.std_objects,
            )
        });

        // Clear is_branching flag
        block.push_store(
            own_label.is_branching.expression,
            body_data
                .expressions
                .append_constant(body_data.std_objects.naga_bool.const_false),
        );

        parents.push(own_label);

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
            arguments,
            results,
            block,
            stack,
            exit_state: ControlFlowState::Continue,
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

    /// Webassembly allows breaks/returns/jumps mid-block, while naga doesn't. This is a sink method used
    /// after an unconditional branch when we need to discard everything left in a function. It eats up to,
    /// but not including, the next *balanced* end instruction
    fn eat_to_end(instructions: &mut Peekable<impl Iterator<Item = OperatorByProposal>>) {
        let mut depth = 0;
        while let Some(instruction) = instructions.peek() {
            match instruction {
                OperatorByProposal::ControlFlow(cfo) => match cfo {
                    ControlFlowOperator::End => {
                        if depth == 0 {
                            return;
                        }
                        depth -= 1
                    }
                    ControlFlowOperator::Block { .. }
                    | ControlFlowOperator::If { .. }
                    | ControlFlowOperator::Loop { .. } => depth += 1,
                    _ => {}
                },
                _ => {}
            }

            instructions.next();
        }
    }

    /// Peeks the top n items and stores them in the n return variables
    fn push_store_stack_in_results(&mut self) {
        for (value, local) in self.stack.iter().rev().zip(self.results.iter().rev()) {
            self.block.push_store(local.expression, *value);
        }
    }

    /// Stores true in the variables giving whether the top n parents should break
    fn push_store_break_top_n_parents(
        block: &mut naga::Block,
        parents: &Vec<BlockLabel>,
        true_expression: naga::Handle<naga::Expression>,
        relative_depth: u32,
    ) {
        for i in 0..=relative_depth {
            let i = usize::try_from(i).expect("branching depth must fit within CPU word size");
            let i = parents.len() - i - 1;
            let parent = parents
                .get(i)
                .expect("validation must ensure that branch does not escape parents");

            let is_branching = parent.is_branching.expression;
            block.push_store(is_branching, true_expression);
        }
    }

    fn do_br(&mut self, relative_depth: u32) -> ControlFlowState {
        Self::push_store_break_top_n_parents(
            &mut self.block,
            &self.parents,
            self.body_data.naga_true_expression,
            relative_depth,
        );
        ControlFlowState::UnconditionalBranch {
            relative_unconditional_depth: relative_depth,
            relative_additional_conditional_depth: 0,
        }
    }

    fn do_br_if(&mut self, relative_depth: u32) -> ControlFlowState {
        let mut accept = naga::Block::default();
        Self::push_store_break_top_n_parents(
            &mut accept,
            &self.parents,
            self.body_data.naga_true_expression,
            relative_depth,
        );

        // Build condition
        let value = self.pop();
        let wasm_false = self.body_data.wasm_false_expression;
        let condition = naga_expr!(self => value != wasm_false);

        self.block.push(
            naga::Statement::If {
                condition,
                accept,
                reject: naga::Block::default(),
            },
            naga::Span::UNDEFINED,
        );

        ControlFlowState::ConditionalBranch { relative_depth }
    }

    fn do_block(
        &mut self,
        blockty: wasmparser::BlockType,
        instructions: &mut Peekable<impl Iterator<Item = OperatorByProposal>>,
    ) -> build::Result<ControlFlowState> {
        let block_type = BlockType::from_parsed(blockty, &self.body_data.module_data.types);

        // Get args
        let mut args = Vec::new();
        for _ in 0..block_type.arguments.len() {
            args.push(self.stack.pop().expect("validation ensures args exist"));
        }
        args.reverse();

        // Make new block, temporarily moving out of this
        let mut inner_block = naga::Block::default();
        let inner_active_block = ActiveBlock::new(
            &mut inner_block,
            block_type,
            self.body_data,
            self.parents.clone(),
        );

        // Write args
        inner_active_block.assign_arguments(&mut self.block, args);

        let (inner_active_block, end) = inner_active_block.populate_straight(instructions)?;
        debug_assert_eq!(end, EndInstruction::End);
        let (results, exit_state) = inner_active_block.finish();

        self.block
            .push(naga::Statement::Block(inner_block), naga::Span::UNDEFINED);

        // Extract results
        for result in results {
            let expr = naga_expr!(self => Load(result.expression));
            self.stack.push(expr);
        }

        Ok(exit_state)
    }

    fn do_if(
        &mut self,
        blockty: wasmparser::BlockType,
        instructions: &mut Peekable<impl Iterator<Item = OperatorByProposal>>,
    ) -> build::Result<ControlFlowState> {
        let value = self.pop();
        let wasm_false = self.body_data.wasm_false_expression;
        let condition = naga_expr!(self => value != wasm_false);

        let block_type = BlockType::from_parsed(blockty, &self.body_data.module_data.types);

        // Get args
        let mut args = Vec::new();
        for _ in 0..block_type.arguments.len() {
            args.push(self.stack.pop().expect("validation ensures args exist"));
        }
        args.reverse();

        // Make new accept block, temporarily moving out of this
        let mut accept = naga::Block::default();
        let inner_active_accept_block = ActiveBlock::new(
            &mut accept,
            block_type.clone(),
            self.body_data,
            self.parents.clone(),
        );

        // Write args
        inner_active_accept_block.assign_arguments(&mut self.block, args.clone());
        let (inner_active_accept_block, end) =
            inner_active_accept_block.populate_straight(instructions)?;
        let (results, accept_exit_state) = inner_active_accept_block.finish();

        let mut reject = naga::Block::default();
        let (results, reject_exit_state) = if end == EndInstruction::Else {
            // Make new reject block, temporarily moving out of this
            let mut inner_active_reject_block = ActiveBlock::new(
                &mut reject,
                block_type,
                &mut self.body_data,
                self.parents.clone(),
            );

            // Write args
            inner_active_reject_block.assign_arguments(&mut self.block, args);
            // Overwrite results to unify if statement results
            inner_active_reject_block.results = results;
            let (inner_active_reject_block, end) =
                inner_active_reject_block.populate_straight(instructions)?;
            debug_assert_eq!(end, EndInstruction::End);
            let (results, reject_exit_state) = inner_active_reject_block.finish();

            (results, reject_exit_state)
        } else {
            // To match if branch, write popped args to results
            for (arg, result) in args.into_iter().zip_eq(results.iter()) {
                reject.push_store(result.expression, arg)
            }

            // If we don't have an else branch, we continue straight through
            (results, ControlFlowState::Continue)
        };

        self.block.push(
            naga::Statement::If {
                condition,
                accept,
                reject,
            },
            naga::Span::UNDEFINED,
        );

        // Extract unified results
        for result in results {
            let expr = naga_expr!(self => Load(result.expression));
            self.stack.push(expr);
        }

        Ok(ControlFlowState::union(
            accept_exit_state,
            reject_exit_state,
        ))
    }

    fn do_loop(
        &mut self,
        blockty: wasmparser::BlockType,
        instructions: &mut Peekable<impl Iterator<Item = OperatorByProposal>>,
    ) -> build::Result<ControlFlowState> {
        let block_type = BlockType::from_parsed(blockty, &self.body_data.module_data.types);

        // Get args
        let mut args = Vec::new();
        for _ in 0..block_type.arguments.len() {
            args.push(self.stack.pop().expect("validation ensures args exist"));
        }
        args.reverse();

        // Make new block, temporarily moving out of this
        let mut inner_block = naga::Block::default();
        let inner_active_block = ActiveBlock::new(
            &mut inner_block,
            block_type,
            self.body_data,
            self.parents.clone(),
        );

        // Write args
        inner_active_block.assign_arguments(&mut self.block, args);
        let (inner_active_block, end) = inner_active_block.populate_looping(instructions)?;
        debug_assert_eq!(end, EndInstruction::End);
        let (results, exit_state) = inner_active_block.finish();

        // Loops exit if they don't continue
        inner_block.push(naga::Statement::Break, naga::Span::UNDEFINED);

        self.block.push(
            naga::Statement::Loop {
                body: inner_block,
                continuing: naga::Block::default(),
                break_if: None,
            },
            naga::Span::UNDEFINED,
        );

        // Extract results
        for result in results {
            let expr = naga_expr!(self => Load(result.expression));
            self.stack.push(expr);
        }

        Ok(exit_state)
    }

    fn do_return(&mut self) -> ControlFlowState {
        self.body_data.push_return(&mut self.block, &mut self.stack);
        ControlFlowState::UnconditionalBranch {
            relative_unconditional_depth: u32::MAX, // Gone
            relative_additional_conditional_depth: 0,
        }
    }

    /// Populates a block using the callbacks provided
    pub(crate) fn populate(
        mut self,
        instructions: &mut Peekable<impl Iterator<Item = OperatorByProposal>>,
        // What to do when we're branching with relative distance 0
        on_r0_branching: impl Fn(&mut ActiveBlock, &mut naga::Block),
        // What to do when we're branching with relative distance >0
        on_rp_branching: impl Fn(&mut ActiveBlock, &mut naga::Block),
    ) -> build::Result<(Self, EndInstruction)> {
        let end_instruction = loop {
            let operation = self.eat_basic_block(instructions)?;
            let state = match operation {
                ControlFlowOperator::End => {
                    break EndInstruction::End;
                }
                // Just interpret as the end of a block, since for an if...else...end that's what it is
                ControlFlowOperator::Else => {
                    break EndInstruction::Else;
                }
                ControlFlowOperator::Br { relative_depth } => self.do_br(relative_depth),
                ControlFlowOperator::BrIf { relative_depth } => self.do_br_if(relative_depth),
                ControlFlowOperator::Block { blockty } => self.do_block(blockty, instructions)?,
                ControlFlowOperator::If { blockty } => self.do_if(blockty, instructions)?,
                ControlFlowOperator::Loop { blockty } => self.do_loop(blockty, instructions)?,
                ControlFlowOperator::Return => self.do_return(),
                ControlFlowOperator::BrTable {
                    targets,
                    default_target,
                } => unimplemented!(),
                ControlFlowOperator::Call { function_index } => unimplemented!(),
                ControlFlowOperator::CallIndirect {
                    type_index,
                    table_index,
                    table_byte,
                } => unimplemented!(),
            };

            // No need to do any control flow shenanigans if there's no control flow happening
            if matches!(state, ControlFlowState::Continue) {
                continue;
            }

            // Write current stack to variables, since we might be leaving this block here
            self.push_store_stack_in_results();

            // Decide what to do if we are branching
            let mut r0_branching = naga::Block::default();
            on_r0_branching(&mut self, &mut r0_branching);

            // Decide what to do if the parent is also branching - useful for loops where we break rather than continue
            let mut rp_branching = naga::Block::default();
            on_rp_branching(&mut self, &mut rp_branching);

            match state {
                ControlFlowState::Continue => {}
                ControlFlowState::UnconditionalBranch {
                    relative_unconditional_depth: 0,
                    ..
                } => {
                    self.block.append(&mut r0_branching);
                    Self::eat_to_end(instructions);
                }
                ControlFlowState::UnconditionalBranch { .. } => {
                    self.block.append(&mut rp_branching);
                    Self::eat_to_end(instructions);
                }
                ControlFlowState::ConditionalBranch { relative_depth } => {
                    // Get r0 break expression
                    let r0_break_expr = self
                        .parents
                        .last()
                        .expect("every block pushes its own label")
                        .is_branching
                        .expression;
                    let r0_condition = naga_expr!(&mut self => Load(r0_break_expr));
                    // Reset our break expression (after loading the value it contains)
                    self.block
                        .push_store(r0_break_expr, self.body_data.naga_false_expression);

                    // Get rp break expression
                    if relative_depth > 0 && self.parents.len() > 1 {
                        if let Some(parent_label) = self.parents.get(self.parents.len() - 2) {
                            let rp_break_expr = parent_label.is_branching.expression;
                            let rp_condition = naga_expr!(&mut self => Load(rp_break_expr));
                            r0_branching = naga::Block::from_vec(vec![naga::Statement::If {
                                condition: rp_condition,
                                accept: rp_branching,
                                reject: r0_branching,
                            }]);
                        }
                    }

                    self.block.push(
                        naga::Statement::If {
                            condition: r0_condition,
                            accept: r0_branching,
                            reject: naga::Block::default(),
                        },
                        naga::Span::UNDEFINED,
                    );

                    // Then get the reject block back to continue populating
                    self.block = match self.block.last_mut().expect("just pushed something") {
                        naga::Statement::If { reject, .. } => reject,
                        _ => unreachable!(
                            "just pushed an if statement, so last item must be an if statement"
                        ),
                    }
                }
            }

            self.exit_state = ControlFlowState::union(self.exit_state, state.downgrade())
        };

        Ok((self, end_instruction))
    }

    /// Populates a non-looping (straight) block
    pub(crate) fn populate_straight(
        self,
        instructions: &mut Peekable<impl Iterator<Item = OperatorByProposal>>,
    ) -> build::Result<(Self, EndInstruction)> {
        self.populate(instructions, |_, _| {}, |_, _| {})
    }

    /// Populates a looping block
    pub(crate) fn populate_looping(
        self,
        instructions: &mut Peekable<impl Iterator<Item = OperatorByProposal>>,
    ) -> build::Result<(Self, EndInstruction)> {
        self.populate(
            instructions,
            |active, block| {
                // Read results back to arguments
                for (dst, src) in active
                    .arguments
                    .clone()
                    .into_iter()
                    .zip(active.results.clone().into_iter())
                {
                    let value = naga_expr!(active => Load(src.expression));
                    block.push_store(dst.expression, value);
                }
                block.push(naga::Statement::Continue, naga::Span::UNDEFINED);
            },
            |_, block| {
                block.push(naga::Statement::Break, naga::Span::UNDEFINED);
            },
        )
    }

    pub(crate) fn finish(mut self) -> (Vec<FnLocal>, ControlFlowState) {
        // Write back stack
        self.push_store_stack_in_results();

        // Put label back
        let own_label = self
            .parents
            .pop()
            .expect("every block pushes then pops its own label");
        self.body_data.unused_labels.push(own_label);

        // Deconstruct
        let Self {
            results,
            exit_state,
            body_data: _,
            parents: _,
            block: _,
            arguments: _,
            stack: _,
        } = self;
        return (results, exit_state);
    }

    /// Fills instructions until some control flow instruction
    fn eat_basic_block(
        &mut self,
        instructions: &mut impl Iterator<Item = OperatorByProposal>,
    ) -> build::Result<ControlFlowOperator> {
        let mut last_op = None;
        while let Some(operation) = instructions.next() {
            match operation {
                OperatorByProposal::ControlFlow(found_last_op) => {
                    last_op = Some(found_last_op.clone());
                    break;
                }
                OperatorByProposal::MVP(mvp_op) => mvp::eat_mvp_operator(self, mvp_op)?,
                OperatorByProposal::Threads(threads_op) => {
                    threads::eat_threads_operator(self, threads_op)?
                }
                OperatorByProposal::Exceptions(_)
                | OperatorByProposal::TailCall(_)
                | OperatorByProposal::ReferenceTypes(_)
                | OperatorByProposal::SignExtension(_)
                | OperatorByProposal::SaturatingFloatToInt(_)
                | OperatorByProposal::BulkMemory(_)
                | OperatorByProposal::SIMD(_)
                | OperatorByProposal::RelaxedSIMD(_)
                | OperatorByProposal::FunctionReferences(_)
                | OperatorByProposal::MemoryControl(_) => {
                    return Err(BuildError::UnsupportedInstructionError {
                        instruction_opcode: operation.opcode(),
                    })
                }
            };
        }

        return Ok(last_op.expect("blocks should be balanced"));
    }
}

// Methods used by instruction implementations to modify a working block
impl<'b, 'd> ActiveBlock<'b, 'd> {
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

    /// Takes a byte address in shared memory space and calculates the address in disjoint memory space. I.e. calculates
    /// `(address / STRIDE) * instance_count + STRIDE * instance_id + (address % STRIDE)`
    fn disjoint_memory_address(
        &mut self,
        shared_address: naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression> {
        let stride_bytes = naga_expr!(self => U32(MEMORY_STRIDE_WORDS * 4));

        let constants_buffer = self.std_objects().bindings.constants;
        let constants_buffer = self.body_data.expressions.append_global(constants_buffer);
        let instance_count =
            naga_expr!(self => Load(constants_buffer[const TOTAL_INVOCATIONS_CONSTANT_INDEX]));

        let instance_id = self.std_objects().instance_id;
        let instance_id = self.body_data.expressions.append_global(instance_id);
        let instance_id = naga_expr!(self => Load(instance_id));

        naga_expr!(self => ((shared_address / stride_bytes) * {instance_count}) + (stride_bytes * instance_id) + (shared_address % stride_bytes))
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

    /// Pops one argument, then calls a function and pushes the result
    fn pop_one_push_call_mono(
        &mut self,
        function: naga::Handle<naga::Function>,
    ) -> build::Result<()> {
        let value = self.pop();
        self.push_call(function, vec![value])
    }

    /// Used when calling a memory function, by popping the address, adding the memory arg as constants and pushing a call to the memory function
    fn pop_one_push_call_mem_func(
        &mut self,
        memarg: wasmparser::MemArg,
        memory_function: naga::Handle<naga::Function>,
    ) -> Result<(), BuildError> {
        let wasmparser::MemArg {
            offset,
            memory,
            // Alignment has no semantic influence, it is a performance hint
            align: _,
            max_align: _,
        } = memarg;

        let offset = u32::try_from(offset)
            .map_err(|_| BuildError::BoundsExceeded(ExceededComponent::MemArgOffset))?;

        let memory = naga_expr!(self => U32(memory));

        let address = self.pop();
        let address = naga_expr!(self => address + U32(offset));

        if self.body_data.uses_disjoint_memory {
            let address = self.disjoint_memory_address(address);
        }

        self.push_call(memory_function, vec![memory, address])
    }

    /// Used when calling a memory function, by popping the address and operand, adding the memory arg as constants
    /// and calling the memory function (discarding the return value). This method also optionally incorporates the
    /// invocation ID to the memory operation, if `disjoint_memory` is enabled.
    fn pop_two_call_mem_func(
        &mut self,
        memarg: wasmparser::MemArg,
        memory_function: naga::Handle<naga::Function>,
    ) -> Result<(), BuildError> {
        let wasmparser::MemArg {
            offset,
            memory,
            // Alignment has no semantic influence, it is a performance hint
            align: _,
            max_align: _,
        } = memarg;

        let offset = u32::try_from(offset)
            .map_err(|_| BuildError::BoundsExceeded(ExceededComponent::MemArgOffset))?;

        let value = self.pop();

        let memory = naga_expr!(self => U32(memory));

        let address = self.pop();
        let address = naga_expr!(self => address + U32(offset));

        if self.body_data.uses_disjoint_memory {
            let address = self.disjoint_memory_address(address);
        }

        self.append(naga::Statement::Call {
            function: memory_function,
            arguments: vec![memory, address, value],
            result: None,
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

impl<'b, 'd> ShaderPart for ActiveBlock<'b, 'd> {
    fn parts(
        &mut self,
    ) -> (
        &mut naga::Arena<naga::Constant>,
        &mut naga::Arena<naga::Expression>,
        &mut naga::Block,
    ) {
        (
            self.body_data.constants,
            self.body_data.expressions,
            self.block,
        )
    }
}
