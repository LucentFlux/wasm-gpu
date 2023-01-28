use super::{BodyState, BuildError};
use crate::module::operation::MVPOperator;

pub(super) fn eat_mvp_operator(
    state: &mut BodyState<'_>,
    operator: &MVPOperator,
) -> Result<(), BuildError> {
    match operator {
        MVPOperator::Nop => {}
        _ => unimplemented!(),
    }

    return Ok(());
}
