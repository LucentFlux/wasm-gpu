use std::marker::PhantomData;

use crate::{build, module_ext::FunctionExt, std_objects::StdObjects, BuildError};

use super::{
    body_gen::FunctionBodyInformation, mvp::eat_mvp_operator, ActiveFunction,
    ActiveInternalFunction,
};
use wasm_opcodes::OperatorByProposal;
use wasmtime_environ::Trap;

/// Everything used while running through basic block instructions to make naga functions.
/// Parsing most instructions involves a straight run of values and operations. That straight run (or basic block)
/// without control flow is eaten and converted by this. Since there is no control flow, expressions can be
/// compounded without auxilliary assignments.
pub(crate) struct ActiveBasicBlock<'b, 'f: 'b, 'm: 'f> {
    // Global shader data, e.g. types or constants
    function: &'b mut ActiveInternalFunction<'f, 'm>,
    _lifetimes: PhantomData<&'b mut &'f mut &'m mut ()>,

    // naga::Handles into the above module
    func_body_info: FunctionBodyInformation<'b>,

    // What we're building into to make the function body
    stack: Vec<naga::Handle<naga::Expression>>,
    statements: Vec<naga::Statement>,
}

impl<'b, 'f: 'b, 'm: 'f> ActiveBasicBlock<'b, 'f, 'm> {
    /// Pushes an expression on to the current stack
    pub(crate) fn push(&mut self, value: naga::Expression) -> naga::Handle<naga::Expression> {
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
            self.append(naga::Statement::Emit(naga::Range::new_from_bounds(
                handle, handle,
            )));
        }

        return handle;
    }

    /// Pops an expression from the current stack
    pub(crate) fn pop(&mut self) -> naga::Handle<naga::Expression> {
        self.stack
            .pop()
            .expect("wasm validation asserts local stack will not be empty")
    }

    /// Peeks an expression from the current stack, for operations like local.tee
    pub(crate) fn peek(&self) -> naga::Handle<naga::Expression> {
        self.stack
            .last()
            .expect("wasm validation asserts local stack will not be empty")
            .clone()
    }

    /// Appends a statement to the current block
    pub(crate) fn append(&mut self, statement: naga::Statement) {
        self.statements.push(statement);
    }

    /// Calls trap, recording the given flag
    pub(crate) fn append_trap(&mut self, trap_id: Trap) -> build::Result<()> {
        let val = self
            .function()
            .get_module()
            .std_objs
            .trap_values
            .get(&Some(trap_id))
            .expect("unknown trap id")
            .clone();

        let trap_val_handle = self.function.get_mut().append_constant(val);

        let trap_fn_handle = self.function.get_module().std_objs.trap_fn;

        self.statements.push(naga::Statement::Call {
            function: trap_fn_handle,
            arguments: vec![trap_val_handle],
            result: None,
        });

        Ok(())
    }

    pub(crate) fn make_wasm_constant(
        &mut self,
        value: wasm_types::Val,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        self.function.make_wasm_constant(value)
    }

    /// Populates until a control flow instruction
    pub(crate) fn build(
        function: &mut ActiveInternalFunction<'f, 'm>,
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

    pub(crate) fn function<'a>(&'a self) -> &'a ActiveInternalFunction<'f, 'm>
    where
        'f: 'a,
    {
        self.function
    }

    pub(crate) fn local_ptr(&self, local_index: u32) -> naga::Handle<naga::Expression> {
        self.function().data.locals.get(local_index).expression
    }

    pub(crate) fn std_objects(&self) -> &StdObjects {
        &self.function().get_module().std_objs
    }

    /// Calls a function and pushes the result of the call onto the stack
    pub(crate) fn call(
        &mut self,
        function: naga::Handle<naga::Function>,
        arguments: Vec<naga::Handle<naga::Expression>>,
    ) -> Result<(), BuildError> {
        let result = self.push(naga::Expression::CallResult(function));

        self.append(naga::Statement::Call {
            function,
            arguments,
            result: Some(result),
        });

        Ok(())
    }
}
