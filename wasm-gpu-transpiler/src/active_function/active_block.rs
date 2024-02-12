use std::iter::Peekable;

use itertools::Itertools;
use naga_ext::{naga_expr, BlockContext, ConstantsExt};
use wasm_opcodes::{proposals::ControlFlowOperator, OperatorByProposal};
use wasmparser::ValType;
use wasmtime_environ::Trap;

use crate::{
    build, linked_stack::LinkedStack, std_objects::StdObjects, typed::Val, BuildError,
    ExceededComponent, FuncAccessible, FunctionModuleData, Tuneables, MEMORY_STRIDE_WORDS,
};

use self::block_label::{BlockLabel, BlockLabelGen};

use super::{
    locals::{FnLocal, FnLocals},
    results::WasmFnResTy,
};

mod block_label;
mod mvp;
mod sign_extension;
mod simd;
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

/// Data shared across an entire function body, shared by many blocks and mutable references are built into as blocks are populated.
pub(crate) struct BodyData<'a> {
    std_objects: &'a StdObjects,
    tuneables: &'a Tuneables,

    // Taken from the definition of the function
    accessible: &'a FuncAccessible,
    module_data: &'a FunctionModuleData,
    return_type: &'a Option<WasmFnResTy>,
    locals: &'a FnLocals,

    /// Used when branching through many scopes at once.
    block_label_set: BlockLabelGen,

    /// To avoid checking if we have trapped too often, this counter is used and incremented on each loop iteration
    trap_check_counter: naga::Handle<naga::Expression>,
}

impl<'a> BodyData<'a> {
    pub(crate) fn new(
        accessible: &'a FuncAccessible,
        module_data: &'a FunctionModuleData,
        return_type: &'a Option<WasmFnResTy>,
        locals: &'a FnLocals,
        ctx: &mut BlockContext<'_>,
        std_objects: &'a StdObjects,
        tuneables: &'a Tuneables,
    ) -> Self {
        let zero = ctx.literal_expr_from(0u32);
        let trap_check_counter = ctx.new_local(
            "trap_check_counter",
            std_objects.preamble.word_ty,
            Some(zero),
        );
        let trap_check_counter = ctx.local_expr(trap_check_counter);

        let block_label_set = BlockLabelGen::new(ctx);

        Self {
            tuneables,
            accessible,
            module_data,
            return_type,
            locals,
            std_objects,
            trap_check_counter,
            block_label_set,
        }
    }

    /// Return results by popping from the current stack, or just returns if the function has no such return values.
    fn push_return(&self, ctx: BlockContext<'_>, stack: &mut Vec<naga::Handle<naga::Expression>>) {
        if let Some(return_type) = &self.return_type {
            let required_values = return_type.components().len();
            let mut results = Vec::new();
            for _ in 0..required_values {
                results.push(stack.pop().expect("validation ensures that enough values were on the stack when return is invoked"))
            }
            results.reverse();
            return_type.push_return(ctx, results);
        } else {
            ctx.void_return()
        }
    }

    /// Used in the outer function scope once the final block has been completed to emit the final return
    pub(crate) fn push_final_return(&self, mut ctx: BlockContext<'_>, results: Vec<FnLocal>) {
        let mut result_expressions = Vec::new();

        for local in results {
            // Load values as expressions
            let ptr = local.expression;
            let expression = naga_expr!(&mut ctx => Load(ptr));
            result_expressions.push(expression)
        }

        self.push_return(ctx, &mut result_expressions);
    }
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum EndInstruction {
    End,
    Else,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct ControlFlowState {
    /// The depth at which it is guaranteed that we will be unconditionally branching. A conservative lower bound.
    pub(crate) lower_unconditional_depth: Option<u32>,
    /// The lower depth at which it is possible that we will be conditionally branching. A conservative upper bound.
    pub(crate) upper_conditional_depth: Option<u32>,
    /// The upper depth at which it is possible that we will be conditionally branching. A conservative lower bound.
    pub(crate) lower_conditional_depth: Option<u32>,
}
impl ControlFlowState {
    /// Finds the state given that control flow may have come from the left or right branch.
    fn union(lhs: ControlFlowState, rhs: ControlFlowState) -> ControlFlowState {
        ControlFlowState {
            lower_unconditional_depth: <Option<u32>>::min(
                lhs.lower_unconditional_depth,
                rhs.lower_unconditional_depth,
            ),
            upper_conditional_depth: <Option<u32>>::min(
                lhs.upper_conditional_depth,
                rhs.upper_conditional_depth,
            ),
            lower_conditional_depth: <Option<u32>>::max(
                lhs.lower_conditional_depth,
                rhs.lower_conditional_depth,
            ),
        }
    }

    /// Finds the state given that control flow flowed through the first, and then the second block.
    fn concat(first: ControlFlowState, second: ControlFlowState) -> ControlFlowState {
        match first.lower_unconditional_depth {
            Some(_) => first,
            None => {
                let lower_conditional_depth = <Option<u32>>::min(
                    first.upper_conditional_depth,
                    second.upper_conditional_depth,
                );
                ControlFlowState {
                    lower_unconditional_depth: <Option<u32>>::min(
                        second.lower_unconditional_depth,
                        lower_conditional_depth,
                    ),
                    upper_conditional_depth: lower_conditional_depth,
                    lower_conditional_depth: <Option<u32>>::max(
                        first.lower_conditional_depth,
                        second.lower_conditional_depth,
                    ),
                }
            }
        }
    }

    /// Gives the control flow state of this when viewed from one block above. I.e. a branch of
    /// depth 1 gives an branch of depth 0, a branch with unconditional depth of 0 and conditional
    /// depth of 2 gives a branch with a conditional depth of 1
    fn decrement(&self) -> ControlFlowState {
        Self {
            lower_unconditional_depth: self
                .lower_unconditional_depth
                .and_then(|v| v.checked_sub(1)),
            upper_conditional_depth: self.upper_conditional_depth.and_then(|v| v.checked_sub(1)),
            lower_conditional_depth: self.lower_conditional_depth.and_then(|v| v.checked_sub(1)),
        }
    }
}

impl Default for ControlFlowState {
    fn default() -> Self {
        Self {
            lower_unconditional_depth: None,
            lower_conditional_depth: None,
            upper_conditional_depth: None,
        }
    }
}

macro_rules! unary {
    ($state:ident, $ty:ident::$fn:ident) => {
        $state.pop_one_push_call_mono($state.std_objects().$ty.$fn)
    };
}
use unary;

macro_rules! binary {
    ($state:ident, $ty:ident::$fn:ident) => {
        $state.pop_two_push_call_bi($state.std_objects().$ty.$fn)
    };
}
use binary;

macro_rules! mem_load {
    ($state:ident, $memarg:ident, $ty:ident::$fn:ident) => {
        $state.pop_one_push_call_mem_func($memarg, $state.std_objects().$ty.$fn)
    };
}
use mem_load;

macro_rules! mem_store {
    ($state:ident, $memarg:ident, $ty:ident::$fn:ident) => {
        $state.pop_two_call_mem_func($memarg, $state.std_objects().$ty.$fn)
    };
}
use mem_store;

/// When we're building the body of a function, we don't actually need to know what the function is.
/// We can simply build the block objects, and keep a mutable reference to the expressions within the
/// function, and go from there.
pub(crate) struct ActiveBlock<'b> {
    /// The block we are populating into.
    pub(crate) ctx: BlockContext<'b>,

    /// The immutable state from the function we're within.
    body_data: &'b BodyData<'b>,

    labels: LinkedStack<'b, BlockLabel>,

    /// Blocks take arguments off of the stack and push back results, like inline functions. To manage these
    /// values in a control-flow independent way (e.g. for looping or branching blocks) we take the parameters
    /// as locals and pass them back as locals to be read.
    arguments: Vec<FnLocal>,
    results: Vec<FnLocal>,

    stack: Vec<naga::Handle<naga::Expression>>,

    exit_state: ControlFlowState,
}

impl<'b> ActiveBlock<'b> {
    pub(super) fn new(
        mut ctx: BlockContext<'b>,
        block_type: BlockType,
        body_data: &'b BodyData<'b>,
        parents: Option<&'b LinkedStack<BlockLabel>>,
    ) -> Self {
        // Set up block-level data
        let own_label = body_data.block_label_set.get_label(&mut ctx);

        let arguments = FnLocal::append_all_wasm_to(
            format!("block_{}_arguments", own_label.id()),
            &mut ctx,
            &body_data.std_objects,
            block_type.arguments,
        );
        let results = FnLocal::append_all_wasm_to(
            format!("block_{}_results", own_label.id()),
            &mut ctx,
            &body_data.std_objects,
            block_type.results,
        );

        let labels = match parents {
            Some(parents) => parents.push(own_label),
            None => LinkedStack::new(own_label),
        };

        // First thing we do is load arguments into stack
        let mut stack = Vec::new();
        for argument in &arguments {
            let load_expression = naga_expr!(&mut ctx => Load(argument.expression));
            stack.push(load_expression);
        }

        Self {
            ctx,
            body_data,
            labels,
            arguments,
            results,
            stack,
            exit_state: ControlFlowState::default(),
        }
    }

    pub(crate) fn assign_arguments(&mut self, values: Vec<naga::Handle<naga::Expression>>) {
        for (value, local) in values.into_iter().zip_eq(&self.arguments) {
            self.ctx.store(local.expression, value);
        }
    }

    /// Webassembly allows breaks/returns/jumps mid-block, while naga doesn't. This is a sink method used
    /// after an unconditional branch when we need to discard everything left in a function. It eats up to,
    /// but not including, the next *balanced* end instruction
    fn eat_to_end<'a: 'c, 'c>(
        instructions: &mut Peekable<impl Iterator<Item = &'c OperatorByProposal<'a>>>,
    ) {
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
            self.ctx.store(local.expression, *value);
        }
    }

    /// Stores true in the variables giving whether the top n parents should break
    fn push_store_break_top_n_parents(
        ctx: &mut BlockContext<'_>,
        labels: &LinkedStack<BlockLabel>,
        relative_depth: u32,
    ) {
        let relative_depth =
            usize::try_from(relative_depth).expect("branching depth must fit within CPU word size");
        for label in labels.peek_n(relative_depth) {
            label.set(ctx);
        }
    }

    fn do_br(&mut self, relative_depth: u32) -> ControlFlowState {
        Self::push_store_break_top_n_parents(&mut self.ctx, &self.labels, relative_depth);
        ControlFlowState {
            lower_unconditional_depth: Some(relative_depth),
            lower_conditional_depth: Some(relative_depth),
            upper_conditional_depth: Some(relative_depth),
        }
    }

    fn do_br_if(&mut self, relative_depth: u32) -> ControlFlowState {
        // Build condition
        let value = self.pop();
        let wasm_false = self.body_data.std_objects.preamble.wasm_bool.const_false;
        let condition = naga_expr!(self => value != Constant(wasm_false));

        self.ctx.test(condition).then(|mut ctx| {
            Self::push_store_break_top_n_parents(&mut ctx, &self.labels, relative_depth);
        });

        ControlFlowState {
            lower_unconditional_depth: None,
            upper_conditional_depth: Some(relative_depth),
            lower_conditional_depth: Some(relative_depth),
        }
    }

    fn do_block<'a: 'c, 'c>(
        &mut self,
        blockty: wasmparser::BlockType,
        instructions: &mut Peekable<impl Iterator<Item = &'c OperatorByProposal<'a>>>,
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
        let mut inner_active_block = ActiveBlock::new(
            (&mut self.ctx).into(),
            block_type,
            self.body_data,
            Some(&self.labels),
        );

        // Write args
        inner_active_block.assign_arguments(args);

        let end = inner_active_block.populate_straight(instructions)?;
        debug_assert_eq!(end, EndInstruction::End);
        let (results, exit_state) = inner_active_block.finish();

        // Extract results
        for result in results {
            let expr = naga_expr!(self => Load(result.expression));
            self.stack.push(expr);
        }

        Ok(exit_state)
    }

    fn do_if<'a: 'c, 'c>(
        &mut self,
        blockty: wasmparser::BlockType,
        instructions: &mut Peekable<impl Iterator<Item = &'c OperatorByProposal<'a>>>,
    ) -> build::Result<ControlFlowState> {
        let value = self.pop();
        let wasm_false = self.body_data.std_objects.preamble.wasm_bool.const_false;
        let condition = naga_expr!(self => value != Constant(wasm_false));

        let block_type = BlockType::from_parsed(blockty, &self.body_data.module_data.types);

        // Get args
        let mut args = Vec::new();
        for _ in 0..block_type.arguments.len() {
            args.push(self.stack.pop().expect("validation ensures args exist"));
        }
        args.reverse();

        let mut end = EndInstruction::End;
        let mut results = vec![];
        let mut exit_state = ControlFlowState::default();
        let test = self.ctx.test(condition).try_then(|ctx| {
            // Make new accept block
            let mut accept_block =
                ActiveBlock::new(ctx, block_type.clone(), self.body_data, Some(&self.labels));

            // Write args
            accept_block.assign_arguments(args.clone());

            // Perform body
            end = accept_block.populate_straight(instructions)?;
            (results, exit_state) = accept_block.finish();

            Ok(())
        })?;

        if end == EndInstruction::Else {
            test.otherwise(|ctx| {
                // Make new reject block
                let mut reject_block =
                    ActiveBlock::new(ctx, block_type, self.body_data, Some(&self.labels));

                // Write args
                reject_block.assign_arguments(args);
                // Overwrite results to unify if statement results
                reject_block.results = results.clone();
                let end = reject_block.populate_straight(instructions)?;
                debug_assert_eq!(end, EndInstruction::End);
                let (_, reject_exit_state) = reject_block.finish();

                exit_state = ControlFlowState::union(exit_state, reject_exit_state);

                Ok(())
            })?;
        } else {
            test.otherwise(|mut ctx| {
                // To match if branch, write popped args to results
                for (arg, result) in args.into_iter().zip_eq(results.iter()) {
                    ctx.store(result.expression, arg)
                }
            });
        };

        // Extract unified results
        for result in results {
            let expr = naga_expr!(self => Load(result.expression));
            self.stack.push(expr);
        }

        Ok(exit_state)
    }

    fn do_loop<'a: 'c, 'c>(
        &mut self,
        blockty: wasmparser::BlockType,
        instructions: &mut Peekable<impl Iterator<Item = &'c OperatorByProposal<'a>>>,
    ) -> build::Result<ControlFlowState> {
        let trap_state = self.body_data.std_objects.preamble.trap_state;
        let trap_state = naga_expr!(self => Global(trap_state));

        let block_type = BlockType::from_parsed(blockty, &self.body_data.module_data.types);

        // Get args
        let mut args = Vec::new();
        for _ in 0..block_type.arguments.len() {
            args.push(self.stack.pop().expect("validation ensures args exist"));
        }
        args.reverse();

        // Make loop block
        let (results, exit_state) = self.ctx.cycle(|mut ctx| {
            let mut loop_body = ActiveBlock::new(
                ctx.reborrow(),
                block_type,
                self.body_data,
                Some(&self.labels),
            );

            // To avoid infinite loops on trapped modules, periodically check if we have trapped
            let trapped_condition = naga_expr!(&mut loop_body => Load(trap_state) != U32(0));
            loop_body
                .ctx
                .test(trapped_condition)
                .then(|ctx| ctx.stop_loop());

            // Write args
            loop_body.assign_arguments(args);

            // Do loop body
            let end = loop_body.populate_looping(instructions)?;
            debug_assert_eq!(end, EndInstruction::End);
            let res = loop_body.finish();

            // Loops exit if they don't continue
            ctx.stop_loop();

            Ok(res)
        })?;

        // Extract results
        for result in results {
            let expr = naga_expr!(self => Load(result.expression));
            self.stack.push(expr);
        }

        Ok(exit_state)
    }

    fn do_return(&mut self) -> ControlFlowState {
        self.body_data
            .push_return((&mut self.ctx).into(), &mut self.stack);

        ControlFlowState {
            lower_unconditional_depth: Some(u32::MAX), // Gone
            upper_conditional_depth: Some(u32::MAX),
            lower_conditional_depth: Some(u32::MAX),
        }
    }

    /// Populates a block using the callbacks provided
    pub(crate) fn populate<'a: 'c, 'c>(
        &mut self,
        instructions: &mut Peekable<impl Iterator<Item = &'c OperatorByProposal<'a>>>,
        // What to do when we're branching with relative distance 0
        on_r0_branching: impl Fn(&mut ActiveBlock<'_>),
        // What to do when we're branching with relative distance >0
        on_rp_branching: impl Fn(&mut ActiveBlock<'_>),
    ) -> build::Result<EndInstruction> {
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
                ControlFlowOperator::Br { relative_depth } => self.do_br(*relative_depth),
                ControlFlowOperator::BrIf { relative_depth } => self.do_br_if(*relative_depth),
                ControlFlowOperator::Block { blockty } => self.do_block(*blockty, instructions)?,
                ControlFlowOperator::If { blockty } => self.do_if(*blockty, instructions)?,
                ControlFlowOperator::Loop { blockty } => self.do_loop(*blockty, instructions)?,
                ControlFlowOperator::Return => self.do_return(),
                ControlFlowOperator::BrTable { targets } => unimplemented!(),
                ControlFlowOperator::Call { function_index } => unimplemented!(),
                ControlFlowOperator::CallIndirect {
                    type_index,
                    table_index,
                    table_byte,
                } => unimplemented!(),
            };

            self.exit_state = ControlFlowState::concat(self.exit_state, state.decrement());

            // No need to do any control flow shenanigans if there's no control flow happening
            if state.lower_conditional_depth.is_none() {
                continue;
            }

            // Write current stack to variables, since we might be leaving this block here
            self.push_store_stack_in_results();

            match state {
                ControlFlowState {
                    lower_unconditional_depth: None,
                    lower_conditional_depth: None,
                    ..
                } => {}
                ControlFlowState {
                    lower_unconditional_depth: Some(0),
                    ..
                } => {
                    on_r0_branching(self);
                }
                ControlFlowState {
                    lower_unconditional_depth: Some(_),
                    ..
                } => {
                    on_rp_branching(self);
                }
                ControlFlowState {
                    lower_unconditional_depth: None,
                    lower_conditional_depth: Some(relative_depth),
                    ..
                } => {
                    // Get r0 break expression
                    let top_label = self.labels.peek();

                    top_label
                        .if_is_set(&mut self.ctx)
                        .then(|mut ctx| {
                            // Reset our break expression if it were set.
                            top_label.unset(&mut ctx);

                            if let Some(parent_label) = self.labels.peek_nth(1) {
                                parent_label
                                    .if_is_set(&mut ctx)
                                    .then(|mut ctx| {
                                        let mut rp_branching = self.reborrow(ctx);
                                        on_rp_branching(&mut rp_branching);
                                    })
                                    .otherwise(|mut ctx| {
                                        let mut r0_branching = self.reborrow(ctx);
                                        on_r0_branching(&mut r0_branching);
                                    });
                            } else {
                                let mut r0_branching = self.reborrow(ctx);
                                on_r0_branching(&mut r0_branching);
                            }
                        })
                        .otherwise(|ctx| {
                            // If we aren't branching, continue with the remainder of the body we're generating.
                            self.ctx = ctx;
                        });
                }
            }

            // If we're done with this block, leave
            if state.lower_unconditional_depth.is_some() {
                Self::eat_to_end(instructions);
            }
        };

        Ok(end_instruction)
    }

    /// Populates a non-looping (straight) block
    pub(crate) fn populate_straight<'a: 'c, 'c>(
        &mut self,
        instructions: &mut Peekable<impl Iterator<Item = &'c OperatorByProposal<'a>>>,
    ) -> build::Result<EndInstruction> {
        self.populate(instructions, |_| {}, |_| {})
    }

    /// Populates a looping block
    pub(crate) fn populate_looping<'a: 'c, 'c>(
        &mut self,
        instructions: &mut Peekable<impl Iterator<Item = &'c OperatorByProposal<'a>>>,
    ) -> build::Result<EndInstruction> {
        self.populate(
            instructions,
            |active| {
                // Read results back to arguments
                for (dst, src) in active
                    .arguments
                    .clone()
                    .into_iter()
                    .zip(active.results.clone().into_iter())
                {
                    let value = naga_expr!(active => Load(src.expression));
                    active.ctx.store(dst.expression, value);
                }
                active.ctx.resume_loop();
            },
            |active| {
                active.ctx.stop_loop();
            },
        )
    }

    pub(crate) fn finish(mut self) -> (Vec<FnLocal>, ControlFlowState) {
        // Write back stack
        self.push_store_stack_in_results();

        // Deconstruct
        let Self {
            ctx,
            body_data,
            labels,
            arguments,
            results,
            stack,
            exit_state,
        } = self;

        return (results, exit_state);
    }

    /// Fills instructions until some control flow instruction
    fn eat_basic_block<'a: 'c, 'c, 's>(
        &'s mut self,
        instructions: &mut impl Iterator<Item = &'c OperatorByProposal<'a>>,
    ) -> build::Result<&'c ControlFlowOperator> {
        let mut last_op = None;
        while let Some(operation) = instructions.next() {
            match operation {
                OperatorByProposal::ControlFlow(found_last_op) => {
                    last_op = Some(found_last_op);
                    break;
                }
                OperatorByProposal::MVP(mvp_op) => mvp::eat_mvp_operator(self, mvp_op)?,
                OperatorByProposal::SignExtension(sign_ext_op) => {
                    sign_extension::eat_sign_extension_operator(self, sign_ext_op)?
                }
                OperatorByProposal::SIMD(simd_op) => simd::eat_simd_operator(self, simd_op)?,
                OperatorByProposal::Threads(threads_op) => {
                    threads::eat_threads_operator(self, threads_op)?
                }
                OperatorByProposal::Exceptions(_)
                | OperatorByProposal::TailCall(_)
                | OperatorByProposal::ReferenceTypes(_)
                | OperatorByProposal::SaturatingFloatToInt(_)
                | OperatorByProposal::BulkMemory(_)
                | OperatorByProposal::RelaxedSIMD(_)
                | OperatorByProposal::FunctionReferences(_)
                | OperatorByProposal::MemoryControl(_)
                | OperatorByProposal::GC(_) => {
                    return Err(BuildError::UnsupportedInstructionError {
                        instruction_opcode: operation.opcode(),
                    })
                }
            };
        }

        return Ok(last_op.expect("blocks should be balanced"));
    }

    /// Given a context, borrows this and gives a new active block for the given time
    fn reborrow<'a>(&'a self, ctx: BlockContext<'a>) -> ActiveBlock<'a> {
        ActiveBlock { ctx, ..*self }
    }
}

// Methods used by instruction implementations to modify a working block
impl<'b> ActiveBlock<'b> {
    /// Pushes an expression on to the current stack
    fn push(&mut self, value: naga::Expression) -> naga::Handle<naga::Expression> {
        let handle = self.ctx.append_expr(value);
        self.stack.push(handle);

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

    /// Calls trap, recording the given flag
    fn append_trap(&mut self, trap_id: Trap) -> build::Result<()> {
        let mut ctx = self.into();
        self.body_data
            .std_objects
            .preamble
            .trap_values
            .emit_set_trap(
                &mut ctx,
                trap_id,
                self.body_data.std_objects.preamble.trap_state,
            );

        Ok(())
    }

    fn push_const_val(&mut self, value: Val) -> build::Result<()> {
        let val_type = value.get_type();
        let init = self
            .body_data
            .std_objects
            .make_wasm_constant(self.ctx.const_expressions, value)?;
        let constant = self
            .ctx
            .constants
            .append_anonymous(self.body_data.std_objects.get_val_type(val_type), init);
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
    /// `(address / STRIDE) * invocations_count + STRIDE * instance_id + (address % STRIDE)`
    fn disjoint_memory_address(
        &mut self,
        shared_address: naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression> {
        let stride_bytes = naga_expr!(self => U32(MEMORY_STRIDE_WORDS * 4));

        let invocations_count_global = self.std_objects().preamble.invocations_count;
        let invocations_count = naga_expr!(self => Load(Global(invocations_count_global)));

        let instance_id_global = self.std_objects().preamble.instance_id;
        let instance_id = naga_expr!(self => Load(Global(instance_id_global)));

        naga_expr!(self => ((shared_address / stride_bytes) * {invocations_count}) + (stride_bytes * instance_id) + (shared_address % stride_bytes))
    }

    /// Calls a function and pushes the result of the call onto the stack
    fn push_call(
        &mut self,
        function: naga::Handle<naga::Function>,
        arguments: Vec<naga::Handle<naga::Expression>>,
    ) -> build::Result<()> {
        let result = self.ctx.call_get_return(function, arguments);
        self.stack.push(result);

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
        memarg: &wasmparser::MemArg,
        memory_function: naga::Handle<naga::Function>,
    ) -> Result<(), BuildError> {
        let wasmparser::MemArg {
            offset,
            memory,
            // Alignment has no semantic influence, it is a performance hint
            align: _,
            max_align: _,
        } = memarg;

        let offset = u32::try_from(*offset)
            .map_err(|_| BuildError::BoundsExceeded(ExceededComponent::MemArgOffset))?;

        let memory = naga_expr!(self => U32(*memory));

        let address = self.pop();
        let mut address = naga_expr!(self => address + U32(offset));

        if self.body_data.tuneables.disjoint_memory {
            address = self.disjoint_memory_address(address);
        }

        self.push_call(memory_function, vec![memory, address])
    }

    /// Used when calling a memory function, by popping the address and operand, adding the memory arg as constants
    /// and calling the memory function (discarding the return value). This method also optionally incorporates the
    /// invocation ID to the memory operation, if `disjoint_memory` is enabled.
    fn pop_two_call_mem_func(
        &mut self,
        memarg: &wasmparser::MemArg,
        memory_function: naga::Handle<naga::Function>,
    ) -> Result<(), BuildError> {
        let wasmparser::MemArg {
            offset,
            memory,
            // Alignment has no semantic influence, it is a performance hint
            align: _,
            max_align: _,
        } = memarg;

        let offset = u32::try_from(*offset)
            .map_err(|_| BuildError::BoundsExceeded(ExceededComponent::MemArgOffset))?;

        let value = self.pop();

        let memory = naga_expr!(self => U32(*memory));

        let address = self.pop();
        let mut address = naga_expr!(self => address + U32(offset));

        if self.body_data.tuneables.disjoint_memory {
            address = self.disjoint_memory_address(address);
        }

        self.ctx
            .call_void(memory_function, vec![memory, address, value]);

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

impl<'a, 'b> From<&'a mut ActiveBlock<'b>> for BlockContext<'a> {
    fn from(value: &'a mut ActiveBlock<'b>) -> Self {
        value.ctx
    }
}
