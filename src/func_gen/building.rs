//! Some functions and tools to streamline manually building naga functions from data
use super::ActiveFunction;

pub(crate) fn make_inner_func_result(ty: naga::Handle<naga::Type>) -> Option<naga::FunctionResult> {
    Some(naga::FunctionResult { ty, binding: None })
}
