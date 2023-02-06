use crate::{func::assembled_module::BuildError, module::operation::OperatorByProposal};

use super::{basic_block_gen::build_basic_block, body_gen::FunctionBodyInformation};

/// A straight run-through block, where branches jump forwards. Eats tokens up to an End token.
pub fn populate_block(
    entry_stack: Vec<naga::Handle<naga::Expression>>,
    instructions: &mut impl Iterator<Item = OperatorByProposal>,
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    body_info: FunctionBodyInformation,
) -> Result<Vec<naga::Handle<naga::Expression>>, BuildError> {
    let (stack_result, mut body_result) = build_basic_block(
        entry_stack,
        instructions,
        module,
        function_handle,
        body_info,
    )?;

    module
        .functions
        .get_mut(function_handle)
        .body
        .append(&mut body_result);

    return Ok(stack_result);
}
