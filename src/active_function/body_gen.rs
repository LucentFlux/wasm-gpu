use std::collections::HashMap;
use wasm_opcodes::OperatorByProposal;

use crate::{
    active_function::results::WasmFnResTy,
    build,
    wasm_front::{FuncAccessible, FunctionModuleData},
    FuncUnit,
};

use super::{block_gen::populate_block, ActiveFunction};

pub(crate) struct FunctionBodyInformation<'a> {
    pub(crate) accessible: &'a FuncAccessible,
    pub(crate) module_data: &'a FunctionModuleData,
    pub(crate) locals_ptrs_map: &'a HashMap<u32, naga::Handle<naga::Expression>>,
}

pub(super) fn populate_base_fn_body<'f, 'm: 'f, F: ActiveFunction<'f, 'm>>(
    working: &mut F,
    parsed: &FuncUnit,
    generated_locals_map: &HashMap<u32, naga::Handle<naga::LocalVariable>>,
    result_type: &Option<WasmFnResTy>,
) -> build::Result<()> {
    let mut locals_ptrs_map = HashMap::new();

    {
        let function = working.get_mut();

        // Convert generated locals to pointers
        for (i, local) in generated_locals_map {
            let local_ptr = function.expressions.append(
                naga::Expression::LocalVariable(*local),
                naga::Span::UNDEFINED,
            );
            locals_ptrs_map.insert(*i, local_ptr);
        }

        // Get parameters as locals
        for (i_param, _) in parsed.data.ty.params().into_iter().enumerate() {
            let i_param = u32::try_from(i_param).expect("small number of parameters");
            let arg_ptr = function.expressions.append(
                naga::Expression::FunctionArgument(i_param),
                naga::Span::UNDEFINED,
            );

            let popped = locals_ptrs_map.insert(i_param, arg_ptr);
            assert!(
                popped.is_none(),
                "function locals map should not have overlapping locals and parameters"
            )
        }
    }

    // Parse instructions
    let accessible = &parsed.accessible;
    let module_data = parsed.data.module_data.as_ref();
    let mut instructions = parsed.data.operators.iter().map(OperatorByProposal::clone);
    let body_info = FunctionBodyInformation {
        accessible,
        module_data,
        locals_ptrs_map: &locals_ptrs_map,
    };
    let entry_stack = vec![];
    let exit_stack = populate_block(working, &mut instructions, entry_stack, body_info)?;

    // Return results
    if let Some(result_type) = result_type {
        let func = working.get_mut();
        result_type.push_return(func, exit_stack);
    }

    return Ok(());
}
