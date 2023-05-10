use std::collections::HashMap;

use wasmtime_environ::Trap;

use super::std_objects_gen;
use crate::traps::ALL_TRAPS;
use crate::{trap_to_u32, TRAP_FLAG_INDEX};
use naga_ext::{declare_function, naga_expr, BlockExt, ModuleExt};

// fn<buffer>(value: u32) -> !
pub(super) fn gen_trap_function<Ps: crate::std_objects::GenerationParameters>(
    module: &mut naga::Module,
    word_ty: std_objects_gen::Word,
    invocation_global: naga::Handle<naga::GlobalVariable>,
    flags_buffer: naga::Handle<naga::GlobalVariable>,
) -> crate::build::Result<naga::Handle<naga::Function>> {
    let (function_handle, trap_value) = declare_function! {
        module => fn trap(trap_id: word_ty)
    };

    let invocation_id = naga_expr!(module, function_handle => Load(Global(invocation_global)));
    let write_word_loc = naga_expr!(module, function_handle => Global(flags_buffer)[invocation_id][const TRAP_FLAG_INDEX]);

    module
        .fn_mut(function_handle)
        .body
        .push_store(write_word_loc, trap_value);

    // Then kill
    // Except we can't kill compute shaders
    // module.fn_mut(function_handle).body.push_kill();

    Ok(function_handle)
}

fn make_trap_constant(
    module: &mut naga::Module,
    trap: Option<Trap>,
) -> naga::Handle<naga::Constant> {
    let trap_id = trap_to_u32(trap);

    let name = match trap {
        Some(trap) => format!("{:?}", trap).to_uppercase(),
        None => "UNSET".to_owned(),
    };

    module.constants.append(
        naga::Constant {
            name: Some(format!("TRAP_{:?}", name)),
            specialization: None,
            inner: naga::ConstantInner::Scalar {
                width: 4,
                value: naga::ScalarValue::Uint(trap_id as u64),
            },
        },
        naga::Span::UNDEFINED,
    )
}

pub(super) fn make_trap_constants<Ps: super::GenerationParameters>(
    module: &mut naga::Module,
) -> crate::build::Result<std_objects_gen::TrapValues> {
    let mut traps = HashMap::new();

    let handle = make_trap_constant(module, None);
    traps.insert(None, handle);

    for trap in ALL_TRAPS {
        let handle = make_trap_constant(module, Some(trap));
        traps.insert(Some(trap), handle);
    }

    return Ok(traps);
}
