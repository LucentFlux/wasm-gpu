use std::collections::HashMap;
use wasm_opcodes::OperatorByProposal;

use crate::{
    assembled_module::BuildError,
    bindings_gen::BindingHandles,
    func::{FuncAccessible, FuncInstance, FunctionModuleData},
};

use super::{block_gen::populate_block, WasmNagaFnRes, WorkingFunction};

pub(crate) struct FunctionBodyInformation<'a> {
    pub(crate) accessible: &'a FuncAccessible,
    pub(crate) module_data: &'a FunctionModuleData,
    pub(crate) locals_ptrs_map: &'a HashMap<u32, naga::Handle<naga::Expression>>,
    pub(crate) bindings: &'a BindingHandles,
}

pub(super) fn populate_body<'a, F: WorkingFunction<'a>>(
    working: &mut F,
    parsed: &FuncInstance,
    generated_locals_map: &HashMap<u32, naga::Handle<naga::LocalVariable>>,
    bindings: &BindingHandles,
    result_type: &Option<WasmNagaFnRes>,
) -> Result<(), BuildError> {
    let mut locals_ptrs_map = HashMap::new();

    {
        let function = working.get_fn_mut();

        // Convert generated locals to pointers
        for (i, local) in generated_locals_map {
            let local_ptr = function.expressions.append(
                naga::Expression::LocalVariable(*local),
                naga::Span::UNDEFINED,
            );
            locals_ptrs_map.insert(*i, local_ptr);
        }

        // Get parameters as locals
        for (i_param, _) in parsed.func_data.ty.params().into_iter().enumerate() {
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
    let module_data = parsed.func_data.module_data.as_ref();
    let mut instructions = parsed
        .func_data
        .operators
        .iter()
        .map(OperatorByProposal::clone);
    let body_info = FunctionBodyInformation {
        accessible,
        module_data,
        locals_ptrs_map: &locals_ptrs_map,
        bindings,
    };
    let entry_stack = vec![];
    let exit_stack = populate_block(entry_stack, &mut instructions, working, body_info)?;

    // Return results
    if let Some(result_type) = result_type {
        let func = working.get_fn_mut();
        let struct_build = func.expressions.append(
            naga::Expression::Compose {
                ty: result_type.handle.clone(),
                components: exit_stack,
            },
            naga::Span::UNDEFINED,
        );

        func.body.push(
            naga::Statement::Emit(naga::Range::new_from_bounds(struct_build, struct_build)),
            naga::Span::UNDEFINED,
        );
        func.body.push(
            naga::Statement::Return {
                value: Some(struct_build),
            },
            naga::Span::UNDEFINED,
        );
    }

    return Ok(());
}
