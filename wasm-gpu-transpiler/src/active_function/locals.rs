use std::collections::HashMap;

use crate::{build, std_objects::StdObjects, BuildError, ExceededComponent};
use naga_ext::{BlockExt, ExpressionsExt, LocalsExt};
use wasmparser::ValType;

use super::arguments::WasmFnArgs;

/// A local in a function
#[derive(Clone)]
pub(crate) struct FnLocal {
    /// The type of the function argument
    pub(crate) ty: naga::Handle<naga::Type>,
    /// The expression giving the parameter in the body of the function
    pub(crate) expression: naga::Handle<naga::Expression>,
}

impl FnLocal {
    pub(crate) fn append_wasm_set_to(
        name_prefix: String,
        local_variables: &mut naga::Arena<naga::LocalVariable>,
        expressions: &mut naga::Arena<naga::Expression>,
        std_objects: &StdObjects,
        local_tys: Vec<ValType>,
    ) -> Vec<Self> {
        local_tys
            .into_iter()
            .enumerate()
            .map(|(i, local_ty)| {
                Self::append_wasm_to(
                    format!("{}_{}", name_prefix, i),
                    local_variables,
                    expressions,
                    std_objects,
                    local_ty,
                )
            })
            .collect()
    }

    pub(crate) fn append_wasm_to(
        name: String,
        local_variables: &mut naga::Arena<naga::LocalVariable>,
        expressions: &mut naga::Arena<naga::Expression>,
        std_objects: &StdObjects,
        local_ty: ValType,
    ) -> Self {
        let ty = std_objects.get_val_type(local_ty);
        let init = std_objects.get_default_value(local_ty);
        Self::append_to(name, local_variables, expressions, ty, Some(init))
    }

    pub(crate) fn append_to(
        name: String,
        local_variables: &mut naga::Arena<naga::LocalVariable>,
        expressions: &mut naga::Arena<naga::Expression>,
        ty: naga::Handle<naga::Type>,
        init: Option<naga::Handle<naga::Constant>>,
    ) -> Self {
        let local = local_variables.new_local(name, ty, init);
        let expression = expressions.append_local(local);
        Self { ty, expression }
    }
}

pub(crate) struct FnLocals {
    locals: HashMap<u32, FnLocal>,
}

impl FnLocals {
    pub(crate) fn append_to(
        function: &mut naga::Function,
        std_objects: &StdObjects,
        parsed_locals: &Vec<(u32, ValType)>,
        parameters: &WasmFnArgs,
    ) -> build::Result<Self> {
        let mut locals = HashMap::new();

        // First insert actual wasm locals
        let local_variables = &mut function.local_variables;
        let expressions = &mut function.expressions;
        let mut i_local = parameters.len() as u32;
        for (local_count, local_ty) in parsed_locals {
            for _ in 0..*local_count {
                locals.insert(
                    i_local,
                    FnLocal::append_wasm_to(
                        format!("wasm_defined_local_{}", i_local),
                        local_variables,
                        expressions,
                        std_objects,
                        *local_ty,
                    ),
                );
                i_local += 1;
            }
        }

        // Then insert parameters as locals, since we need to be able to treat them like they are
        for (i_param, parameter) in parameters.iter().enumerate() {
            let i_param = u32::try_from(i_param)
                .map_err(|_| BuildError::BoundsExceeded(ExceededComponent::ParameterCount))?;
            let local_variables = &mut function.local_variables;
            let expressions = &mut function.expressions;
            let local = FnLocal::append_wasm_to(
                format!("parameter_{}_as_local", i_param),
                local_variables,
                expressions,
                std_objects,
                parameter.ty,
            );

            // Immediately assign value to local
            let parameter_value = parameter.arg.expression_handle;
            function.body.push_store(local.expression, parameter_value);

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
            .expect(&format!("unreferencable local should be caught by validation of wasm module when getting local indexed {}", local_index))
    }
}