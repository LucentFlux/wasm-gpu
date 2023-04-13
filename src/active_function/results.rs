use crate::{active_function::ActiveFunction, build};
use crate::{naga_expr, BuildError, ExceededComponent};
use wasm_types::ValTypeByteCount;
use wasmparser::ValType;

use crate::{
    module_ext::FunctionExt, std_objects::StdObjects, IO_ARGUMENT_ALIGNMENT_WORDS,
    IO_INVOCATION_ALIGNMENT_WORDS,
};

use super::ActiveEntryFunction;

/// A return type for a wasm-originated function
#[derive(Debug, Clone)]
pub(crate) struct WasmFnResTy {
    handle: naga::Handle<naga::Type>,
    wasm_ty: Vec<ValType>,
}

impl WasmFnResTy {
    pub(crate) fn make_type(
        module: &mut naga::Module,
        std_objects: &StdObjects,
        results: &[ValType],
    ) -> Option<Self> {
        if results.len() == 0 {
            return None;
        }

        let mut members = Vec::new();
        let mut offset = 0;
        for (i, ty) in results.into_iter().enumerate() {
            let field = std_objects.get_val_type(*ty);

            members.push(naga::StructMember {
                name: Some(format!("v{}", i + 1)),
                ty: field,
                binding: None,
                offset,
            });
            offset += std_objects.get_val_size_bytes(*ty);
        }

        let naga_ty = module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Struct {
                    members,
                    span: offset,
                },
            },
            naga::Span::UNDEFINED,
        );

        return Some(Self {
            handle: naga_ty,
            wasm_ty: Vec::from(results),
        });
    }

    pub(crate) fn set_return_type(&self, function: &mut naga::Function) {
        function.result = Some(naga::FunctionResult {
            ty: self.handle,
            binding: None,
        })
    }

    /// Calculates the word alignment in memory (i.e. buffer bindings) that these results must occupy.
    /// Useful when saving the arguments of an entry function to an output buffer.
    pub(crate) fn word_alignment(&self) -> u32 {
        let mut word_offset = 0;
        for ty in &self.wasm_ty {
            word_offset +=
                u32::from(ty.byte_count()).next_multiple_of(IO_ARGUMENT_ALIGNMENT_WORDS * 4) / 4;
        }

        return word_offset.next_multiple_of(IO_INVOCATION_ALIGNMENT_WORDS);
    }

    /// Builds a struct of the return type and pushes it as a return expression at the end of the function's
    /// statements.
    pub(crate) fn push_return(
        &self,
        func: &mut naga::Function,
        components: Vec<naga::Handle<naga::Expression>>,
    ) {
        let struct_build = func.append_compose_push_emit(self.handle, components);
        func.push_return(struct_build);
    }

    pub(crate) fn append_store_at<'f, 'm: 'f>(
        &self,
        function: &mut ActiveEntryFunction<'f, 'm>,
        location: naga::Handle<naga::Expression>,
        value: naga::Handle<naga::Expression>,
    ) -> build::Result<()> {
        let mut word_offset = 0;
        for (i_res, val_ty) in self.wasm_ty.iter().enumerate() {
            let location = naga_expr! {function => location + (U32(word_offset))};

            let i_res = u32::try_from(i_res)
                .map_err(|_| BuildError::BoundsExceeded(ExceededComponent::ReturnType))?;
            let result = naga_expr! {function => value[const i_res]};

            let store_fn = function.std_objects().get_write_output_fn(*val_ty);

            function.get_mut().body.push(
                naga::Statement::Call {
                    function: store_fn,
                    arguments: vec![location, result],
                    result: None,
                },
                naga::Span::UNDEFINED,
            );

            word_offset += u32::from(val_ty.byte_count())
                .next_multiple_of(IO_ARGUMENT_ALIGNMENT_WORDS * 4)
                / 4;
        }

        Ok(())
    }
}
