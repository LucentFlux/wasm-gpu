use std::sync::Arc;

use crate::typed::V128;
use crate::{build, std_objects::std_objects_gen};
use naga_ext::{declare_function, naga_expr, BlockExt, ConstantsExt, ModuleExt, TypesExt};

use super::{v128_instance_gen, V128Gen};

fn make_ty(types: &mut naga::UniqueArena<naga::Type>) -> naga::Handle<naga::Type> {
    types.insert_anonymous(naga::TypeInner::Vector {
        size: naga::VectorSize::Quad,
        scalar: naga::Scalar::U32,
    })
}

fn make_const_impl(module: &mut naga::Module, value: V128) -> naga::Handle<naga::Constant> {
    let ty = make_ty(&mut module.types);
    let bytes = value.to_le_bytes();
    let components = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| {
            let word = u32::from_le_bytes(*bytes);
            let word = u32::try_from(word & 0xFFFFFFFF).expect("truncated word always fits in u32");
            module.const_expressions.append_u32(word)
        })
        .collect();
    let init = module.const_expressions.append_compose(ty, components);
    module.constants.append_anonymous(ty, init)
}

/// An implementation of v128s using a 4-vector of u32s. Calling this a Polyfill is a slight stretch
/// since a v128 is almost exactly an ivec4, but some things don't map perfectly so it's a polyfill.
pub(crate) struct PolyfillV128;
impl V128Gen for PolyfillV128 {
    fn gen_ty(
        module: &mut naga::Module,
        _others: super::v128_instance_gen::TyRequirements,
    ) -> build::Result<super::v128_instance_gen::Ty> {
        Ok(make_ty(&mut module.types))
    }

    fn gen_default(
        module: &mut naga::Module,
        others: super::v128_instance_gen::DefaultRequirements,
    ) -> build::Result<super::v128_instance_gen::Default> {
        Ok(make_const_impl(module, V128::from_bits(0)))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _others: super::v128_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::v128_instance_gen::SizeBytes> {
        Ok(16)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        _others: super::v128_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::v128_instance_gen::MakeConst> {
        Ok(Arc::new(Box::new(|module, std_objects, value| {
            Ok(make_const_impl(module, value))
        })))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        others: super::v128_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::v128_instance_gen::ReadInput> {
        gen_read(
            module,
            others.word,
            others.ty,
            others.bindings.input,
            "input",
        )
    }

    fn gen_write_output(
        module: &mut naga::Module,
        others: super::v128_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::v128_instance_gen::WriteOutput> {
        gen_write(
            module,
            others.word,
            others.ty,
            others.bindings.output,
            "output",
        )
    }

    fn gen_read_memory(
        module: &mut naga::Module,
        others: super::v128_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::v128_instance_gen::ReadMemory> {
        gen_read(
            module,
            others.word,
            others.ty,
            others.bindings.memory,
            "memory",
        )
    }

    fn gen_write_memory(
        module: &mut naga::Module,
        others: super::v128_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::v128_instance_gen::WriteMemory> {
        gen_write(
            module,
            others.word,
            others.ty,
            others.bindings.memory,
            "memory",
        )
    }
}

// fn<buffer>(word_address: u32) -> v128
fn gen_read(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    v128_ty: v128_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_v128_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> v128_ty
    };

    let input_ref = naga_expr!(module, function_handle => Global(buffer));

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
    module.fn_mut(function_handle).body.push_return(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: v128)
fn gen_write(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    v128_ty: v128_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_v128_to_{}", buffer_name);
    let (handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: v128_ty)
    };

    let output_ref = naga_expr!(module, handle => Global(buffer));

    let write_word_loc1 = naga_expr!(module, handle => output_ref[word_address]);
    let word1 = naga_expr!(module, handle => value[const 0]);
    let write_word_loc2 = naga_expr!(module, handle => output_ref[word_address + (U32(1))]);
    let word2 = naga_expr!(module, handle => value[const 1]);
    let write_word_loc3 = naga_expr!(module, handle => output_ref[word_address + (U32(2))]);
    let word3 = naga_expr!(module, handle => value[const 2]);
    let write_word_loc4 = naga_expr!(module, handle => output_ref[word_address + (U32(3))]);
    let word4 = naga_expr!(module, handle => value[const 3]);

    module
        .fn_mut(handle)
        .body
        .push_store(write_word_loc1, word1);
    module
        .fn_mut(handle)
        .body
        .push_store(write_word_loc2, word2);
    module
        .fn_mut(handle)
        .body
        .push_store(write_word_loc3, word3);
    module
        .fn_mut(handle)
        .body
        .push_store(write_word_loc4, word4);

    Ok(handle)
}
