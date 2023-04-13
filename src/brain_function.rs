use crate::{
    active_function::InternalFunction,
    function_lookup::FunctionLookup,
    module_ext::{FunctionSignature, ModuleExt},
};

use crate::active_module::ActiveModule;

pub(crate) struct BrainFunction {
    handle: naga::Handle<naga::Function>,
}

impl BrainFunction {
    pub(crate) fn append_declaration_to(module: &mut naga::Module) -> Self {
        let handle = module.new_function(FunctionSignature {
            name: "brain".to_owned(),
            args: vec![],
            result: None,
        });

        Self { handle }
    }

    pub(crate) fn populate(
        &self,
        working_module: &mut ActiveModule,
        stack_functions: &FunctionLookup<InternalFunction>,
    ) {
    }
}
