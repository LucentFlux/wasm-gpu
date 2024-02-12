use wasm_opcodes::proposals::SignExtensionOperator;

use crate::build;

use super::{unary, ActiveBlock};

pub(super) fn eat_sign_extension_operator(
    state: &mut ActiveBlock<'_>,
    operator: &SignExtensionOperator,
) -> build::Result<()> {
    match operator {
        SignExtensionOperator::I32Extend8S => unary!(state, i32::extend_8_s),
        SignExtensionOperator::I32Extend16S => unary!(state, i32::extend_16_s),
        SignExtensionOperator::I64Extend8S => unary!(state, i64::extend_8_s),
        SignExtensionOperator::I64Extend16S => unary!(state, i64::extend_16_s),
        SignExtensionOperator::I64Extend32S => unary!(state, i64::extend_32_s),
    }
}
