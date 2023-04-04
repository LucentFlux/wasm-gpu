use crate::assembled_module::BuildError;
use wasm_opcodes::OperatorByProposal;

use super::{
    basic_block_gen::build_basic_block, body_gen::FunctionBodyInformation, ActiveFunction,
};

/// A straight run-through block, where branches jump forwards. Eats tokens up to an End token.
pub(crate) fn populate_block<'a, F: ActiveFunction<'a>>(
    stack: Vec<naga::Handle<naga::Expression>>,
    instructions: &mut impl Iterator<Item = OperatorByProposal>,
    working: &mut F,
    func_body_info: FunctionBodyInformation,
) -> Result<Vec<naga::Handle<naga::Expression>>, BuildError> {
    let (stack_result, mut body_result) =
        build_basic_block(stack, instructions, working, func_body_info)?;

    working.get_fn_mut().body.append(&mut body_result);

    return Ok(stack_result);
}
