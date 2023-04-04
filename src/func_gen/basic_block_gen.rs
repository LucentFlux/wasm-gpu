use std::marker::PhantomData;

use crate::assembled_module::{build, BuildError};

use super::{body_gen::FunctionBodyInformation, mvp::eat_mvp_operator, ActiveFunction};
use wasm_opcodes::OperatorByProposal;

/// Everything used while running through basic block instructions to make naga functions.
/// Parsing most instructions involves a straight run of values and operations. That straight run (or basic block)
/// without control flow is eaten and converted by this. Since there is no control flow, expressions can be
/// compounded without auxilliary assignments.
pub(crate) struct BasicBlockState<'a, 'b, F: ActiveFunction<'b>> {
    // Global shader data, e.g. types or constants
    working: &'a mut F,
    _f_life: PhantomData<&'b ()>,

    // naga::Handles into the above module
    func_body_info: FunctionBodyInformation<'a>,

    // What we're building into to make the function body
    stack: Vec<naga::Handle<naga::Expression>>,
    statements: Vec<naga::Statement>,

    // Used to emit expressions in naga::Statement::Emit
    first_and_last: Option<(
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Expression>,
    )>,
}

impl<'a, 'b, F: ActiveFunction<'b>> BasicBlockState<'a, 'b, F> {
    /// Pushes an expression on to the current stack
    pub(crate) fn push(&mut self, value: naga::Expression) {
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
            .working
            .get_fn_mut()
            .expressions
            .append(value, naga::Span::UNDEFINED);
        self.stack.push(handle);

        if needs_emitting {
            self.first_and_last = match self.first_and_last {
                Some((first, _)) => Some((first, handle)),
                None => Some((handle, handle)),
            }
        } else {
            self.emit_expressions();
        }
    }

    /// Pops an expression from the current stack
    pub(crate) fn pop(&mut self) -> naga::Handle<naga::Expression> {
        self.stack
            .pop()
            .expect("wasm validation asserts local stack will not be empty")
    }

    pub(crate) fn constant(
        &mut self,
        value: wasm_types::Val,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        self.working.constant(value)
    }

    /// Emits all working expressions as a naga block and clears the current working pool.
    /// Should be called before any statement is pushed to the body.
    pub fn emit_expressions(&mut self) {
        if let Some((first, last)) = self.first_and_last.take() {
            self.statements
                .push(naga::Statement::Emit(naga::Range::new_from_bounds(
                    first, last,
                )))
        }
    }
}

/// Populates until a control flow instruction
pub(crate) fn build_basic_block<'a, F: ActiveFunction<'a>>(
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
        first_and_last: None,
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

    // Ensure we don't have anything dangling on return
    state.emit_expressions();

    return Ok((state.stack, naga::Block::from_vec(state.statements)));
}
