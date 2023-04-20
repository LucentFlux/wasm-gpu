use std::collections::HashMap;

use wasmtime_environ::Trap;

use super::{BufferFnGen, StdObjectsGenerator};
use crate::module_ext::{BlockExt, FunctionExt, ModuleExt};
use crate::std_objects::Generator;
use crate::traps::ALL_TRAPS;
use crate::{declare_function, naga_expr, trap_to_u32, TRAP_FLAG_INDEX};

// fn<buffer>(value: u32) -> !
pub(crate) struct TrapFnGen;
impl BufferFnGen for TrapFnGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> crate::build::Result<naga::Handle<naga::Function>> {
        let trap_ty = others.u32.gen(module, others)?;

        let (function_handle, trap_value) = declare_function! {
            module => fn trap(trap_id: trap_ty)
        };

        let output_ref = module.fn_mut(function_handle).append_global(buffer);
        let write_word_loc =
            naga_expr!(module, function_handle => output_ref[const TRAP_FLAG_INDEX]);
        module
            .fn_mut(function_handle)
            .body
            .push_store(write_word_loc, trap_value);

        // Then kill
        module.fn_mut(function_handle).body.push_kill();

        Ok(function_handle)
    }
}

fn make_trap_constant(
    module: &mut naga::Module,
    trap: Option<Trap>,
) -> naga::Handle<naga::Constant> {
    let trap_id = trap_to_u32(trap);

    module.constants.append(
        naga::Constant {
            name: Some(format!("TRAP_ID_{}", trap_id)),
            specialization: None,
            inner: naga::ConstantInner::Scalar {
                width: 4,
                value: naga::ScalarValue::Uint(trap_id as u64),
            },
        },
        naga::Span::UNDEFINED,
    )
}

#[derive(Default)]
pub(super) struct TrapConstantsGen;
impl Generator for TrapConstantsGen {
    type Generated = HashMap<Option<Trap>, naga::Handle<naga::Constant>>;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        _others: &StdObjectsGenerator<Ps>,
    ) -> crate::build::Result<Self::Generated> {
        let mut traps = HashMap::new();

        let handle = make_trap_constant(module, None);
        traps.insert(None, handle);

        for trap in ALL_TRAPS {
            let handle = make_trap_constant(module, Some(trap));
            traps.insert(Some(trap), handle);
        }

        return Ok(traps);
    }
}
