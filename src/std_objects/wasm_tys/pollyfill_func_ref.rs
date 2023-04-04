use wasm_types::FuncRef;

use crate::{
    assembled_module::{build, ActiveModule},
    declare_function,
    module_ext::{FunctionExt, ModuleExt},
    naga_expr,
    std_objects::{
        std_fns::BufferFnGen,
        std_tys::{TyGen, WasmTyGen},
        Generator, WasmTyImpl,
    },
};

/// An implementation of FuncRefs using the GPU's native u32 type
pub(crate) struct PolyfillFuncRef;
impl WasmTyImpl<FuncRef> for PolyfillFuncRef {
    type TyGen = PolyfillFuncRefTyGen;
    type ReadGen = PolyfillFuncRefReadGen;
    type WriteGen = PolyfillFuncRefWriteGen;
}

pub(crate) struct PolyfillFuncRefTyGen;
impl TyGen for PolyfillFuncRefTyGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> crate::assembled_module::build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: Some("FuncRef".to_owned()),
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for PolyfillFuncRefTyGen {
    type WasmTy = FuncRef;

    fn make_const(
        _ty: naga::Handle<naga::Type>,
        working: &mut ActiveModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(working.module.constants.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner: naga::ConstantInner::Scalar {
                    width: 4,
                    value: naga::ScalarValue::Uint(value.as_u32().unwrap_or(u32::MAX).into()),
                },
            },
            naga::Span::UNDEFINED,
        ))
    }
}

// fn<buffer>(word_address: u32) -> func_ref
pub(crate) struct PolyfillFuncRefReadGen;
impl BufferFnGen for PolyfillFuncRefReadGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let func_ref_ty = others.func_ref.ty.gen(module, others)?;

        let (function_handle, word_address) = declare_function! {
            module => fn read_func_ref(word_address: address_ty) -> func_ref_ty
        };

        let input_ref = module.fn_mut(function_handle).append_global(buffer);

        let read_word = naga_expr!(module, function_handle => input_ref[word_address]);
        let read_value = naga_expr!(module, function_handle => Load(read_word));
        module.fn_mut(function_handle).push_return(read_value);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32, value: func_ref)
pub(crate) struct PolyfillFuncRefWriteGen;
impl BufferFnGen for PolyfillFuncRefWriteGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let func_ref_ty = others.func_ref.ty.gen(module, others)?;

        let (function_handle, word_address, value) = declare_function! {
            module => fn write_func_ref(word_address: address_ty, value: func_ref_ty)
        };

        let output_ref = module.fn_mut(function_handle).append_global(buffer);
        let write_word_loc = naga_expr!(module, function_handle => output_ref[word_address]);
        let word = naga_expr!(module, function_handle => value as Uint);
        module
            .fn_mut(function_handle)
            .push_store(write_word_loc, word);

        Ok(function_handle)
    }
}
