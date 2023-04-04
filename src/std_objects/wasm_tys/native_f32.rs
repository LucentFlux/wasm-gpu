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
use wasm_types::Ieee32;

/// An implementation of f32s using the GPU's native f32 type
pub(crate) struct NativeF32;
impl WasmTyImpl<Ieee32> for NativeF32 {
    type TyGen = NativeF32TyGen;
    type ReadGen = NativeF32ReadGen;
    type WriteGen = NativeF32WriteGen;
}

pub(crate) struct NativeF32TyGen;
impl TyGen for NativeF32TyGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Float,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for NativeF32TyGen {
    type WasmTy = Ieee32;

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
                    value: naga::ScalarValue::Float(value.to_float().into()),
                },
            },
            naga::Span::UNDEFINED,
        ))
    }
}

// fn<buffer>(word_address: u32) -> f32
pub(crate) struct NativeF32ReadGen;
impl BufferFnGen for NativeF32ReadGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let f32_ty = others.f32.ty.gen(module, others)?;

        let (function_handle, word_address) = declare_function! {
            module => fn read_f32(word_address: address_ty) -> f32_ty
        };

        let input_ref = module.fn_mut(function_handle).append_global(buffer);

        let read_word = naga_expr!(module, function_handle => input_ref[word_address]);
        let read_value = naga_expr!(module, function_handle => (Load(read_word)) as Float);
        module.fn_mut(function_handle).push_return(read_value);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32, value: f32)
pub(crate) struct NativeF32WriteGen;
impl BufferFnGen for NativeF32WriteGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let f32_ty = others.f32.ty.gen(module, others)?;

        let (function_handle, word_address, value) = declare_function! {
            module => fn write_f32(word_address: address_ty, value: f32_ty)
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
