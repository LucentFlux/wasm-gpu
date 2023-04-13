use wasm_types::V128;

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

/// An implementation of v128s using a 4-vector of u32s. Calling this a Polyfill is a slight stretch
/// since a v128 is almost exactly an ivec4, but some things don't map perfectly so it's a polyfill.
pub(crate) struct PolyfillV128;
impl WasmTyImpl<V128> for PolyfillV128 {
    type TyGen = PolyfillV128TyGen;
    type DefaultGen = PolyfillV128DefaultGen;
    type ReadGen = PolyfillV128ReadGen;
    type WriteGen = PolyfillV128WriteGen;
}

pub(crate) struct PolyfillV128TyGen;
impl TyGen for PolyfillV128TyGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        _others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: Some("v128".to_owned()),
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Quad,
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }

    fn size_bytes() -> u32 {
        16
    }
}
fn make_const_impl(
    module: &mut naga::Module,
    ty: naga::Handle<naga::Type>,
    value: V128,
) -> naga::Handle<naga::Constant> {
    let bytes = value.to_le_bytes();
    let inner = naga::ConstantInner::Composite {
        ty,
        components: bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| {
                let word = u32::from_le_bytes(*bytes);
                let word =
                    u32::try_from(word & 0xFFFFFFFF).expect("truncated word always fits in u32");
                module.constants.append(
                    naga::Constant {
                        name: None,
                        specialization: None,
                        inner: naga::ConstantInner::Scalar {
                            width: 4,
                            value: naga::ScalarValue::Uint(word.into()),
                        },
                    },
                    naga::Span::UNDEFINED,
                )
            })
            .collect(),
    };
    module.constants.append(
        naga::Constant {
            name: None,
            specialization: None,
            inner,
        },
        naga::Span::UNDEFINED,
    )
}
impl WasmTyGen for PolyfillV128TyGen {
    type WasmTy = V128;

    fn make_const(
        module: &mut naga::Module,
        objects: &StdObjects,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(make_const_impl(module, objects.v128.ty, value))
    }
}

pub(crate) struct PolyfillV128DefaultGen;
impl ConstGen for PolyfillV128DefaultGen {
    fn gen<Ps: crate::std_objects::GenerationParameters>(
        module: &mut naga::Module,
        others: &crate::std_objects::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        let ty = others.v128.ty.gen(module, others)?;
        Ok(make_const_impl(module, ty, V128::from_bits(0)))
    }
}

// fn<buffer>(word_address: u32) -> v128
pub(crate) struct PolyfillV128ReadGen;
impl BufferFnGen for PolyfillV128ReadGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let v128_ty = others.v128.ty.gen(module, others)?;

        let (function_handle, word_address) = declare_function! {
            module => fn read_v128(word_address: address_ty) -> v128_ty
        };

        let input_ref = module.fn_mut(function_handle).append_global(buffer);

        let read_word1 = naga_expr!(module, function_handle => input_ref[word_address]);
        let read_word2 = naga_expr!(module, function_handle => input_ref[word_address + (U32(1))]);
        let read_word3 = naga_expr!(module, function_handle => input_ref[word_address + (U32(2))]);
        let read_word4 = naga_expr!(module, function_handle => input_ref[word_address + (U32(3))]);
        let read_value = naga_expr!(module, function_handle => v128_ty(
            (Load(read_word1)),
            (Load(read_word2)),
            (Load(read_word3)),
            (Load(read_word4))
        ));
        module.fn_mut(function_handle).push_return(read_value);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32, value: v128)
pub(crate) struct PolyfillV128WriteGen;
impl BufferFnGen for PolyfillV128WriteGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = others.u32.gen(module, others)?;
        let v128_ty = others.v128.ty.gen(module, others)?;

        let (handle, word_address, value) = declare_function! {
            module => fn write_v128(word_address: address_ty, value: v128_ty)
        };

        let output_ref = module.fn_mut(handle).append_global(buffer);

        let write_word_loc1 = naga_expr!(module, handle => output_ref[word_address]);
        let word1 = naga_expr!(module, handle => value[const 0]);
        let write_word_loc2 = naga_expr!(module, handle => output_ref[word_address + (U32(1))]);
        let word2 = naga_expr!(module, handle => value[const 1]);
        let write_word_loc3 = naga_expr!(module, handle => output_ref[word_address + (U32(2))]);
        let word3 = naga_expr!(module, handle => value[const 2]);
        let write_word_loc4 = naga_expr!(module, handle => output_ref[word_address + (U32(3))]);
        let word4 = naga_expr!(module, handle => value[const 3]);

        module.fn_mut(handle).push_store(write_word_loc1, word1);
        module.fn_mut(handle).push_store(write_word_loc2, word2);
        module.fn_mut(handle).push_store(write_word_loc3, word3);
        module.fn_mut(handle).push_store(write_word_loc4, word4);

        Ok(handle)
    }
}
