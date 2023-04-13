use wasm_types::ExternRef;

use crate::{
    build, declare_function,
    module_ext::{FunctionExt, ModuleExt},
    naga_expr,
    std_objects::{
        std_consts::ConstGen,
        std_fns::BufferFnGen,
        std_tys::{TyGen, WasmTyGen},
        Generator, StdObjects, WasmTyImpl,
    },
};

/// An implementation of ExternRefs using the GPU's native u32 type
pub(crate) struct PolyfillExternRef;
impl WasmTyImpl<ExternRef> for PolyfillExternRef {
    type TyGen = PolyfillExternRefTyGen;
    type DefaultGen = PolyfillExternRefDefaultGen;
    type ReadGen = PolyfillExternRefReadGen;
    type WriteGen = PolyfillExternRefWriteGen;
}

pub(crate) struct PolyfillExternRefTyGen;
impl TyGen for PolyfillExternRefTyGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: Some("ExternRef".to_owned()),
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }

    fn size_bytes() -> u32 {
        4
    }
}
fn make_const_impl(module: &mut naga::Module, value: ExternRef) -> naga::Handle<naga::Constant> {
    module.constants.append(
        naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar {
                width: 4,
                value: naga::ScalarValue::Uint(value.as_u32().unwrap_or(u32::MAX).into()),
            },
        },
        naga::Span::UNDEFINED,
    )
}
impl WasmTyGen for PolyfillExternRefTyGen {
    type WasmTy = ExternRef;

    fn make_const(
        module: &mut naga::Module,
        _objects: &StdObjects,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(make_const_impl(module, value))
    }
}

pub(crate) struct PolyfillExternRefDefaultGen;
impl ConstGen for PolyfillExternRefDefaultGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(make_const_impl(module, ExternRef::none()))
    }
}

// fn<buffer>(word_address: u32) -> extern_ref
pub(crate) struct PolyfillExternRefReadGen;
impl BufferFnGen for PolyfillExternRefReadGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let extern_ref_ty = others.extern_ref.ty.gen(module, others)?;

        let (function_handle, word_address) = declare_function! {
            module => fn read_extern_ref(word_address: address_ty) -> extern_ref_ty
        };

        let input_ref = module.fn_mut(function_handle).append_global(buffer);

        let read_value = naga_expr!(module, function_handle => Load(input_ref[word_address]));
        module.fn_mut(function_handle).push_return(read_value);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32, value: extern_ref)
pub(crate) struct PolyfillExternRefWriteGen;
impl BufferFnGen for PolyfillExternRefWriteGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let extern_ref_ty = others.extern_ref.ty.gen(module, others)?;

        let (function_handle, word_address, value) = declare_function! {
            module => fn write_extern_ref(word_address: address_ty, value: extern_ref_ty)
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
