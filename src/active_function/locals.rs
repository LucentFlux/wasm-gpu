use std::collections::HashMap;

use wasmparser::ValType;

use crate::{
    module_ext::{FunctionExt, ModuleExt},
    std_objects::StdObjects,
};

/// A local in a function
pub(crate) struct FnLocal {
    /// The type of the function argument
    pub(crate) ty: naga::Handle<naga::Type>,
    /// The expression giving the parameter in the body of the function
    pub(crate) expression: naga::Handle<naga::Expression>,
}

impl FnLocal {
    pub(crate) fn append_to(
        module: &mut naga::Module,
        function: naga::Handle<naga::Function>,
        std_objects: &StdObjects,
        local_ty: ValType,
    ) -> Self {
        let ty = std_objects.get_val_type(local_ty);
        let init = std_objects.get_default_value(local_ty);
        let function = module.fn_mut(function);
        let i_local = function.local_variables.len();
        let local = function.new_local(format! {"local_{}", i_local}, ty, Some(init));
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
    ) -> Self {
        let mut handles = HashMap::new();

        for (i_local, local_ty) in parsed_locals {
            handles.insert(
                *i_local,
                FnLocal::append_to(module, function, std_objects, *local_ty),
            );
        }

        return Self { locals: handles };
    }

    pub(crate) fn iter(&self) -> std::collections::hash_map::Iter<u32, FnLocal> {
        self.locals.iter()
    }
}
