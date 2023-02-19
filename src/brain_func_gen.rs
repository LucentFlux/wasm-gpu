use super::assembled_module::{build, WorkingModule};

pub(crate) fn populate_brain_func(working_module: &mut WorkingModule) -> build::Result<()> {
    let brain_function = working_module
        .module
        .functions
        .get_mut(working_module.brain_function);

    brain_function.name = Some("brain".to_owned());

    Ok(())
}
