use std::marker::PhantomData;

use crate::{build, module_ext::FunctionExt, BuildError};

use super::{body_gen::FunctionBodyInformation, mvp::eat_mvp_operator, ActiveFunction};
use wasm_opcodes::OperatorByProposal;

/// Everything used while running through basic block instructions to make naga functions.
/// Parsing most instructions involves a straight run of values and operations. That straight run (or basic block)
/// without control flow is eaten and converted by this. Since there is no control flow, expressions can be
/// compounded without auxilliary assignments.
pub(crate) struct ActiveBasicBlock<'b, 'f: 'b, 'm: 'f, F: ActiveFunction<'f, 'm>> {
    // Global shader data, e.g. types or constants
    function: &'b mut F,
    _lifetimes: PhantomData<&'b mut &'f mut &'m mut ()>,

    // naga::Handles into the above module
    func_body_info: FunctionBodyInformation<'b>,

    // What we're building into to make the function body
    stack: Vec<naga::Handle<naga::Expression>>,
    statements: Vec<naga::Statement>,
}

impl<'b, 'f: 'b, 'm: 'f, F: ActiveFunction<'f, 'm>> ActiveBasicBlock<'b, 'f, 'm, F> {
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
            .function
            .get_mut()
            .expressions
            .append(value, naga::Span::UNDEFINED);
        self.stack.push(handle);

        if needs_emitting {
            self.function.get_mut().push_emit(handle);
        }
    }

    /// Pops an expression from the current stack
    pub(crate) fn pop(&mut self) -> naga::Handle<naga::Expression> {
        self.stack
            .pop()
            .expect("wasm validation asserts local stack will not be empty")
    }

    pub(crate) fn make_constant(
        &mut self,
        value: wasm_types::Val,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        self.function.make_constant(value)
    }

    /// Populates until a control flow instruction
    pub(crate) fn build(
        function: &mut F,
        instructions: &mut impl Iterator<Item = OperatorByProposal>,
        stack: Vec<naga::Handle<naga::Expression>>,
        func_body_info: FunctionBodyInformation,
    ) -> build::Result<(Vec<naga::Handle<naga::Expression>>, naga::Block)> {
        let mut state = ActiveBasicBlock {
            function,
            func_body_info,
            stack,
            statements: Vec::new(),
            _lifetimes: PhantomData,
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
}
