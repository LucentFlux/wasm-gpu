use std::collections::HashMap;

use crate::{
    func::{
        assembled_module::BuildError, bindings_gen::BindingHandles, call_graph::CallOrder,
        FuncAccessible, FuncInstance, FunctionModuleData,
    },
    module::operation::OperatorByProposal,
};

use super::block_gen::populate_block;

pub struct FunctionBodyInformation<'a> {
    pub accessible: &'a FuncAccessible,
    pub module_data: &'a FunctionModuleData,
    pub locals_ptrs_map: &'a HashMap<u32, naga::Handle<naga::Expression>>,
    pub call_order: &'a CallOrder,
    pub brain_function: naga::Handle<naga::Function>,
    pub bindings: &'a BindingHandles,
}

pub fn populate_body(
    parsed: &FuncInstance,
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    generated_locals_map: &HashMap<u32, naga::Handle<naga::LocalVariable>>,
    call_order: &CallOrder,
    brain_function: naga::Handle<naga::Function>,
    bindings: &BindingHandles,
    result_type: Option<naga::Handle<naga::Type>>,
) -> Result<(), BuildError> {
    let mut locals_ptrs_map = HashMap::new();

    {
        let function = module.functions.get_mut(function_handle);

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
    let accessible = parsed
        .accessible
        .as_deref()
        .expect("function should be linked with module before body construction");
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
        call_order,
        brain_function,
        bindings,
    };
    let entry_stack = vec![];
    let exit_stack = populate_block(
        entry_stack,
        &mut instructions,
        module,
        function_handle,
        body_info,
    )?;

    // Return results
    if let Some(result_type) = result_type {
        let func = module.functions.get_mut(function_handle);
        let struct_build = func.expressions.append(
            naga::Expression::Compose {
                ty: result_type,
                components: exit_stack,
            },
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
