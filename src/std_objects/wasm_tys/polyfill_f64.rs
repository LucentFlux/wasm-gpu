use crate::{
    build, declare_function,
    module_ext::{FunctionExt, ModuleExt},
    naga_expr,
    std_objects::{
        std_consts::ConstGen,
        std_fns::BufferFnGen,
        std_tys::{TyGen, WasmTyGen},
        GenerationParameters, Generator, StdObjects, StdObjectsGenerator, WasmTyImpl,
    },
};

/// An implementation of f64s using a 2-vector of u32s
pub(crate) struct PolyfillF64;
impl WasmTyImpl<f64> for PolyfillF64 {
    type TyGen = PolyfillF64TyGen;
    type DefaultGen = PolyfillF64DefaultGen;
    type ReadGen = PolyfillF64ReadGen;
    type WriteGen = PolyfillF64WriteGen;
}

pub(crate) struct PolyfillF64TyGen;
impl TyGen for PolyfillF64TyGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: Some("f64".to_owned()),
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Bi,
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        return Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED));
    }

    fn size_bytes() -> u32 {
        8
    }
}
impl WasmTyGen for PolyfillF64TyGen {
    type WasmTy = f64;

    fn make_const(
        module: &mut naga::Module,
        objects: &StdObjects,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        let value = i64::from_le_bytes(value.to_le_bytes());
        return Ok(super::make_64_bit_const_from_2vec32(
            objects.f64.ty,
            module,
            value,
        ));
    }
}

pub(crate) struct PolyfillF64DefaultGen;
impl ConstGen for PolyfillF64DefaultGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        let value = i64::from_le_bytes((0.0 as f64).to_le_bytes());
        return Ok(super::make_64_bit_const_from_2vec32(
            others.f64.ty.gen(module, others)?,
            module,
            value,
        ));
    }
}

// fn<buffer>(word_address: u32) -> f64
pub(crate) struct PolyfillF64ReadGen;
impl BufferFnGen for PolyfillF64ReadGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let f64_ty = others.f64.ty.gen(module, others)?;

        let (function_handle, word_address) = declare_function! {
            module => fn read_f64(word_address: address_ty) -> f64_ty
        };

        let input_ref = module.fn_mut(function_handle).append_global(buffer);

        let read_word1 = naga_expr!(module, function_handle => input_ref[word_address]);
        let read_word2 = naga_expr!(module, function_handle => input_ref[word_address + U32(1)]);
        let read_value =
            naga_expr!(module, function_handle => f64_ty(Load(read_word1), Load(read_word2)));
        module.fn_mut(function_handle).push_return(read_value);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32, value: f64)
pub(crate) struct PolyfillF64WriteGen;
impl BufferFnGen for PolyfillF64WriteGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let f64_ty = others.f64.ty.gen(module, others)?;

        let (handle, word_address, value) = declare_function! {
            module => fn write_f64(word_address: address_ty, value: f64_ty)
        };

        let output_ref = module.fn_mut(handle).append_global(buffer);

        let write_word_loc1 = naga_expr!(module, handle => output_ref[word_address]);
        let word1 = naga_expr!(module, handle => value[const 0] as Uint);
        let write_word_loc2 = naga_expr!(module, handle => output_ref[word_address + U32(1)]);
        let word2 = naga_expr!(module, handle => value[const 1] as Uint);

        module.fn_mut(handle).push_store(write_word_loc1, word1);
        module.fn_mut(handle).push_store(write_word_loc2, word2);

        Ok(handle)
    }
}
