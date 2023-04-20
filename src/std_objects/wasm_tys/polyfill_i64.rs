use crate::{
    build, declare_function,
    module_ext::{BlockExt, FunctionExt, ModuleExt},
    naga_expr,
    std_objects::{
        std_fns::BufferFnGen, wasm_tys::WasmTyImpl, GenerationParameters, Generator, StdObjects,
        StdObjectsGenerator,
    },
};

/// An implementation of i64s using a 2-vector of u32s
pub(crate) struct PolyfillI64;
impl WasmTyImpl for PolyfillI64 {
    type WasmTy = i64;

    type TyGen = PolyfillI64TyGen;
    type DefaultGen = PolyfillI64DefaultGen;
    type ReadGen = PolyfillI64ReadGen;
    type WriteGen = PolyfillI64WriteGen;

    fn size_bytes() -> u32 {
        8
    }

    fn make_const(
        module: &mut naga::Module,
        objects: &StdObjects,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(super::make_64_bit_const_from_2vec32(
            objects.i64.base.ty,
            module,
            value,
        ))
    }
}

#[derive(Default)]
pub(crate) struct PolyfillI64TyGen;
impl Generator for PolyfillI64TyGen {
    type Generated = naga::Handle<naga::Type>;

    fn gen<Ps: crate::std_objects::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: Some("i64".to_owned()),
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Bi,
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}

#[derive(Default)]
pub(crate) struct PolyfillI64DefaultGen;
impl Generator for PolyfillI64DefaultGen {
    type Generated = naga::Handle<naga::Constant>;

    fn gen<Ps: crate::std_objects::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        Ok(super::make_64_bit_const_from_2vec32(
            others.i64.base.ty.gen(module, others)?,
            module,
            0,
        ))
    }
}

// fn<buffer>(word_address: u32) -> i64
pub(crate) struct PolyfillI64ReadGen;
impl BufferFnGen for PolyfillI64ReadGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let i64_ty = others.i64.base.ty.gen(module, others)?;

        let (function_handle, word_address) = declare_function! {
            module => fn read_i64(word_address: address_ty) -> i64_ty
        };

        let input_ref = module.fn_mut(function_handle).append_global(buffer);

        let read_word1 = naga_expr!(module, function_handle => input_ref[word_address]);
        let read_word2 = naga_expr!(module, function_handle => input_ref[word_address + (U32(1))]);
        let read_value =
            naga_expr!(module, function_handle => i64_ty((Load(read_word1)), (Load(read_word2))));
        module.fn_mut(function_handle).body.push_return(read_value);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32, value: i64)
pub(crate) struct PolyfillI64WriteGen;
impl BufferFnGen for PolyfillI64WriteGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let i64_ty = others.i64.base.ty.gen(module, others)?;

        let (handle, word_address, value) = declare_function! {
            module => fn write_i64(word_address: address_ty, value: i64_ty)
        };

        let output_ref = module.fn_mut(handle).append_global(buffer);

        let write_word_loc1 = naga_expr!(module, handle => output_ref[word_address]);
        let word1 = naga_expr!(module, handle => (value[const 0]) as Uint);
        let write_word_loc2 = naga_expr!(module, handle => output_ref[word_address + (U32(1))]);
        let word2 = naga_expr!(module, handle => (value[const 1]) as Uint);

        module
            .fn_mut(handle)
            .body
            .push_store(write_word_loc1, word1);
        module
            .fn_mut(handle)
            .body
            .push_store(write_word_loc2, word2);

        Ok(handle)
    }
}
