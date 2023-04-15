use std::collections::HashMap;

use wasmparser::ValType;

use crate::{
    build,
    module_ext::{FunctionExt, ModuleExt},
    std_objects::StdObjects,
    BuildError, ExceededComponent,
};

use super::arguments::WasmFnArgs;

/// A local in a function
pub(crate) struct FnLocal {
    /// The type of the function argument
    pub(crate) ty: naga::Handle<naga::Type>,
    /// The expression giving the parameter in the body of the function
    pub(crate) expression: naga::Handle<naga::Expression>,
}

impl FnLocal {
    pub(crate) fn append_wasm_to(
        module: &mut naga::Module,
        function: naga::Handle<naga::Function>,
        std_objects: &StdObjects,
        local_ty: ValType,
    ) -> Self {
        let ty = std_objects.get_val_type(local_ty);
        let init = std_objects.get_default_value(local_ty);
        Self::append_to(module, function, std_objects, ty, Some(init))
    }

    pub(crate) fn append_to(
        module: &mut naga::Module,
        function: naga::Handle<naga::Function>,
        std_objects: &StdObjects,
        ty: naga::Handle<naga::Type>,
        init: Option<naga::Handle<naga::Constant>>,
    ) -> Self {
        let function = module.fn_mut(function);
        let i_local = function.local_variables.len();
        let local = function.new_local(format! {"local_{}", i_local}, ty, init);
        let expression = function.append_local(local);
        Self { ty, expression }
    }
}

pub(crate) struct FnLocals {
    locals: HashMap<u32, FnLocal>,
}

impl FnLocals {
    pub(crate) fn append_to(
        module: &mut naga::Module,
        function: naga::Handle<naga::Function>,
        std_objects: &StdObjects,
        parsed_locals: &Vec<(u32, ValType)>,
        parameters: &WasmFnArgs,
    ) -> build::Result<Self> {
        let mut locals = HashMap::new();

        // First insert actual wasm locals
        for (i_local, local_ty) in parsed_locals {
            locals.insert(
                *i_local,
                FnLocal::append_wasm_to(module, function, std_objects, *local_ty),
            );
        }

        // Then insert parameters as locals, since we need to be able to treat them like they are
        for (i_param, parameter) in parameters.iter().enumerate() {
            let i_param = u32::try_from(i_param)
                .map_err(|_| BuildError::BoundsExceeded(ExceededComponent::ParameterCount))?;
            let local = FnLocal::append_wasm_to(module, function, std_objects, parameter.ty);

            // Immediately assign value to local
            let parameter_value = parameter.arg.expression_handle;
            let function = module.fn_mut(function);
            function.push_store(local.expression, parameter_value);

            let popped = locals.insert(i_param, local);
            assert!(
                popped.is_none(),
                "function locals map should not have overlapping locals and parameters"
            );
        }

        return Ok(Self { locals });
    }

    pub(crate) fn iter(&self) -> std::collections::hash_map::Iter<u32, FnLocal> {
        self.locals.iter()
    }

    pub(crate) fn get(&self, local_index: u32) -> &FnLocal {
        self.locals
            .get(&local_index)
            .expect("unreferencable local should be caught by validation of wasm module")
    }
}
