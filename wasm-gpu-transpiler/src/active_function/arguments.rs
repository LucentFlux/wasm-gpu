use naga_ext::{naga_expr, ExpressionsExt};
use wasm_types::ValTypeByteCount;
use wasmparser::ValType;

use crate::{std_objects::StdObjects, IO_ARGUMENT_ALIGNMENT_WORDS, IO_INVOCATION_ALIGNMENT_WORDS};

use super::ActiveFunction;

/// An argument in a function
#[derive(Debug, Copy, Clone)]
pub(crate) struct FnArg {
    /// The type of the function argument
    pub(crate) type_handle: naga::Handle<naga::Type>,
    /// The expression giving the parameter in the body of the function
    pub(crate) expression_handle: naga::Handle<naga::Expression>,
}

impl FnArg {
    pub(crate) fn append_to(
        function: &mut naga::Function,
        type_handle: naga::Handle<naga::Type>,
    ) -> Self {
        let i_param = function.arguments.len();
        function.arguments.push(naga::FunctionArgument {
            name: Some(format!("arg_{}", i_param)),
            ty: type_handle,
            binding: None,
        });

        let expression_handle = function.expressions.append_fn_argument(i_param as u32);

        Self {
            type_handle,
            expression_handle,
        }
    }

    pub(crate) fn append_bound_to(
        function: &mut naga::Function,
        type_handle: naga::Handle<naga::Type>,
        binding: naga::Binding,
    ) -> Self {
        let i_param = function.arguments.len();
        function.arguments.push(naga::FunctionArgument {
            name: Some(format!("bound_arg_{}", i_param)),
            ty: type_handle,
            binding: Some(binding),
        });

        let expression_handle = function.expressions.append_fn_argument(i_param as u32);

        Self {
            type_handle,
            expression_handle,
        }
    }

    pub(crate) fn expression(&self) -> naga::Handle<naga::Expression> {
        self.expression_handle.clone()
    }
}

/// An argument in a function given by the wasm source
#[derive(Debug, Copy, Clone)]
pub(crate) struct WasmFnArg {
    pub(crate) arg: FnArg,
    pub(crate) ty: ValType,
}

impl WasmFnArg {
    pub(crate) fn append_read_at<'f, 'm: 'f>(
        &self,
        function: &mut impl ActiveFunction<'f, 'm>,
        location: naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression> {
        let load_fn = function.std_objects().get_read_input_fn(self.ty);

        let entry_fn = function.fn_mut();

        let arg_result = entry_fn
            .expressions
            .append(naga::Expression::CallResult(load_fn), naga::Span::UNDEFINED);
        entry_fn.body.push(
            naga::Statement::Call {
                function: load_fn,
                arguments: vec![location],
                result: Some(arg_result.clone()),
            },
            naga::Span::UNDEFINED,
        );

        return arg_result;
    }
}

/// The set of args given by the wasm source
#[derive(Debug, Clone)]
pub(crate) struct WasmFnArgs {
    args: Vec<WasmFnArg>,
}

impl WasmFnArgs {
    pub(crate) fn append_to(
        function: &mut naga::Function,
        std_objects: &StdObjects,
        param_tys: &[ValType],
    ) -> Self {
        let mut args = Vec::new();
        for ty in param_tys {
            let type_handle = std_objects.get_val_type(*ty);
            let arg = FnArg::append_to(function, type_handle);
            args.push(WasmFnArg { arg, ty: *ty });
        }

        return Self { args };
    }

    /// Calculates the word alignment in memory (i.e. buffer bindings) that these arguments must occupy.
    /// Useful when loading the arguments of an entry function from an input buffer.
    pub(crate) fn word_alignment(&self) -> u32 {
        let mut word_offset = 0;
        for arg in &self.args {
            word_offset += u32::from(arg.ty.byte_count())
                .next_multiple_of(IO_ARGUMENT_ALIGNMENT_WORDS * 4)
                / 4;
        }
        let word_alignment = word_offset.next_multiple_of(IO_INVOCATION_ALIGNMENT_WORDS);
        return word_alignment;
    }

    pub(crate) fn append_read_at<'f, 'm: 'f>(
        &self,
        function: &mut impl ActiveFunction<'f, 'm>,
        location: naga::Handle<naga::Expression>,
    ) -> Vec<naga::Handle<naga::Expression>> {
        let mut arg_results = Vec::new();

        let mut offset = 0u32;
        for arg in &self.args {
            let location = naga_expr!(function => location + U32(offset));

            arg_results.push(arg.append_read_at(function, location));

            offset += u32::from(arg.ty.byte_count())
                .next_multiple_of(IO_ARGUMENT_ALIGNMENT_WORDS * 4)
                / 4;
        }

        return arg_results;
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<WasmFnArg> {
        self.args.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.args.len()
    }
}

/// The things that every entry function has passed in
pub(crate) struct EntryArguments {
    pub(crate) global_id: FnArg,
}

impl EntryArguments {
    pub(crate) fn append_to(function: &mut naga::Function, std_objects: &StdObjects) -> Self {
        let global_id = FnArg::append_bound_to(
            function,
            std_objects.uvec3,
            naga::Binding::BuiltIn(naga::BuiltIn::GlobalInvocationId),
        );

        Self { global_id }
    }
}
