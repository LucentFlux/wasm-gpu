use std::collections::HashMap;
use wasm_opcodes::OperatorByProposal;

use crate::{
    active_function::results::WasmFnResTy,
    build,
    wasm_front::{FuncAccessible, FunctionModuleData},
    FuncUnit,
};

use super::{block_gen::populate_block, ActiveFunction, ActiveInternalFunction};

pub(crate) struct FunctionBodyInformation<'a> {
    pub(crate) accessible: &'a FuncAccessible,
    pub(crate) module_data: &'a FunctionModuleData,
}

pub(super) fn populate_base_fn_body<'f, 'm: 'f>(
    working: &mut ActiveInternalFunction<'f, 'm>,
    parsed: &FuncUnit,
    generated_locals_map: &HashMap<u32, naga::Handle<naga::LocalVariable>>,
    result_type: &Option<WasmFnResTy>,
) -> build::Result<()> {
    // Parse instructions
    let accessible = &parsed.accessible;
    let module_data = parsed.data.module_data.as_ref();
    let mut instructions = parsed.data.operators.iter().map(OperatorByProposal::clone);
    let body_info = FunctionBodyInformation {
        accessible,
        module_data,
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
