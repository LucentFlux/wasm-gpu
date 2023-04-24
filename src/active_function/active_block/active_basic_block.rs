
use std::marker::PhantomData;
use crate::{build, std_objects::StdObjects, BuildError, module_ext::ExpressionsExt};
use super::{ActiveBlock};
use self::mvp::eat_mvp_operator;
use wasm_opcodes::{ControlFlowOperator, OperatorByProposal};
use wasm_types::Val;
use wasmtime_environ::Trap;

/// Everything used while running through basic block instructions to make naga functions.
/// Parsing most instructions involves a straight run of values and operations. That straight run (or basic block)
/// without control flow is eaten and converted by this. Since there is no control flow, expressions can be
/// compounded without auxilliary assignments.
pub(crate) struct ActiveBasicBlock<'a> {
    block: ActiveBlock<'a>,

}

impl<'a> ActiveBasicBlock<'a> {
}
