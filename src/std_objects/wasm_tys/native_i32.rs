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

/// An implementation of i32s using the GPU's native i32 type
pub(crate) struct NativeI32;
impl WasmTyImpl<i32> for NativeI32 {
    type TyGen = NativeI32TyGen;
    type ReadGen = NativeI32ReadGen;
    type WriteGen = NativeI32WriteGen;
}

pub(crate) struct NativeI32TyGen;
impl TyGen for NativeI32TyGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Sint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for NativeI32TyGen {
    type WasmTy = i32;

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
                    value: naga::ScalarValue::Sint(value.into()),
                },
            },
            naga::Span::UNDEFINED,
        ))
    }
}

// fn<buffer>(word_address: u32) -> i32
pub(crate) struct NativeI32ReadGen;
impl BufferFnGen for NativeI32ReadGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let i32_ty = others.i32.ty.gen(module, others)?;

        let (function_handle, word_address) = declare_function! {
            module => fn read_i32(word_address: address_ty) -> i32_ty
        };

        let input_ref = module.fn_mut(function_handle).append_global(buffer);

        let read_word = naga_expr!(module, function_handle => input_ref[word_address]);
        let read_value = naga_expr!(module, function_handle => (Load(read_word)) as Sint);
        module.fn_mut(function_handle).push_return(read_value);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32, value: i32)
pub(crate) struct NativeI32WriteGen;
impl BufferFnGen for NativeI32WriteGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let i32_ty = others.i32.ty.gen(module, others)?;

        let (function_handle, word_address, value) = declare_function! {
            module => fn write_i32(word_address: address_ty, value: i32_ty)
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
