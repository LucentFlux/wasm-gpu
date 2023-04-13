use super::{
    active_basic_block::ActiveBasicBlock, body_gen::FunctionBodyInformation, ActiveFunction,
};
use crate::build;
use wasm_opcodes::OperatorByProposal;

/// A straight run-through block, where branches jump forwards. Eats tokens up to an End token.
pub(crate) fn populate_block<'f, 'm: 'f, F: ActiveFunction<'f, 'm>>(
    working: &mut F,
    instructions: &mut impl Iterator<Item = OperatorByProposal>,
    stack: Vec<naga::Handle<naga::Expression>>,
    func_body_info: FunctionBodyInformation,
) -> build::Result<Vec<naga::Handle<naga::Expression>>> {
    let (stack_result, mut body_result) =
        ActiveBasicBlock::build(working, instructions, stack, func_body_info)?;

    working.get_mut().body.append(&mut body_result);

    return Ok(stack_result);
}
