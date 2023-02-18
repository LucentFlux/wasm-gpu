use std::marker::PhantomData;

use crate::{
    func::assembled_module::{build, BuildError},
    module::operation::OperatorByProposal,
};

use super::{body_gen::FunctionBodyInformation, mvp::eat_mvp_operator, WorkingFunction};

/// Everything used while running through basic block instructions to make naga functions.
/// Parsing most instructions involves a straight run of values and operations. That straight run (or basic block)
/// without control flow is eaten and converted by this. Since there is no control flow, expressions can be
/// compounded without auxilliary assignments.
pub struct BasicBlockState<'a, 'b, F: WorkingFunction<'b>> {
    // Global shader data, e.g. types or constants
    working: &'a mut F,
    _f_life: PhantomData<&'b ()>,

    // naga::Handles into the above module
    func_body_info: FunctionBodyInformation<'a>,

    // What we're building into to make the function body
    stack: Vec<naga::Handle<naga::Expression>>,
    statements: Vec<naga::Statement>,
}

impl<'a, 'b, F: WorkingFunction<'b>> BasicBlockState<'a, 'b, F> {
    /// Pushes an expression on to the current stack
    pub fn push(&mut self, value: naga::Expression) {
        let handle = self
            .working
            .get_fn_mut()
            .expressions
            .append(value, naga::Span::UNDEFINED);
        self.stack.push(handle);
    }

    /// Pops an expression from the current stack
    pub fn pop(&mut self) -> naga::Handle<naga::Expression> {
        self.stack
            .pop()
            .expect("wasm validation asserts local stack will not be empty")
    }

    pub fn constant(&mut self, value: crate::Val) -> build::Result<naga::Handle<naga::Constant>> {
        self.working.constant(value)
    }
}

/// Populates until a control flow instruction
pub fn build_basic_block<'a, F: WorkingFunction<'a>>(
    stack: Vec<naga::Handle<naga::Expression>>,
    instructions: &mut impl Iterator<Item = OperatorByProposal>,
    working: &mut F,
    func_body_info: FunctionBodyInformation,
) -> Result<(Vec<naga::Handle<naga::Expression>>, naga::Block), BuildError> {
    let mut state = BasicBlockState {
        working,
        func_body_info,
        stack,
        statements: Vec::new(),
        _f_life: PhantomData,
    };

    let mut instructions = instructions.peekable();
    while let Some(operation) = instructions.peek() {
        match operation {
            OperatorByProposal::ControlFlow(_) => break,
            OperatorByProposal::MVP(mvp_op) => eat_mvp_operator(&mut state, mvp_op)?,
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

    return Ok((state.stack, naga::Block::from_vec(state.statements)));
}
