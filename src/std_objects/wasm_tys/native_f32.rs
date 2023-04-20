use crate::{
    build, declare_function,
    module_ext::{BlockExt, FunctionExt, ModuleExt},
    naga_expr,
    std_objects::{std_fns::BufferFnGen, wasm_tys::WasmTyImpl, Generator, StdObjects},
};

/// An implementation of f32s using the GPU's native f32 type
pub(crate) struct NativeF32;
impl WasmTyImpl for NativeF32 {
    type WasmTy = f32;

    type TyGen = NativeF32TyGen;
    type DefaultGen = NativeF32DefaultGen;
    type ReadGen = NativeF32ReadGen;
    type WriteGen = NativeF32WriteGen;

    fn size_bytes() -> u32 {
        4
    }

    fn make_const(
        module: &mut naga::Module,
        _objects: &StdObjects,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(make_const_impl(module, value))
    }
}

#[derive(Default)]
pub(crate) struct NativeF32TyGen;
impl Generator for NativeF32TyGen {
    type Generated = naga::Handle<naga::Type>;

    fn gen<Ps: crate::std_objects::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
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
fn make_const_impl(module: &mut naga::Module, value: f32) -> naga::Handle<naga::Constant> {
    module.constants.append(
        naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar {
                width: 4,
                value: naga::ScalarValue::Float(value.into()),
            },
        },
        naga::Span::UNDEFINED,
    )
}

#[derive(Default)]
pub(crate) struct NativeF32DefaultGen;
impl Generator for NativeF32DefaultGen {
    type Generated = naga::Handle<naga::Constant>;

    fn gen<Ps: crate::std_objects::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        Ok(make_const_impl(module, 0.0))
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
        let f32_ty = others.f32.base.ty.gen(module, others)?;

        let (function_handle, word_address) = declare_function! {
            module => fn read_f32(word_address: address_ty) -> f32_ty
        };

        let input_ref = module.fn_mut(function_handle).append_global(buffer);

        let read_word = naga_expr!(module, function_handle => input_ref[word_address]);
        let read_value = naga_expr!(module, function_handle => Load(read_word) as Float);
        module.fn_mut(function_handle).body.push_return(read_value);

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
        let f32_ty = others.f32.base.ty.gen(module, others)?;

        let (function_handle, word_address, value) = declare_function! {
            module => fn write_f32(word_address: address_ty, value: f32_ty)
        };

        let output_ref = module.fn_mut(function_handle).append_global(buffer);
        let write_word_loc = naga_expr!(module, function_handle => output_ref[word_address]);
        let word = naga_expr!(module, function_handle => value as Uint);
        module
            .fn_mut(function_handle)
            .body
            .push_store(write_word_loc, word);

        Ok(function_handle)
    }
}
