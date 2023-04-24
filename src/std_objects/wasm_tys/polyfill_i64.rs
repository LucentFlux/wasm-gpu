use std::sync::Arc;

use crate::{
    build, declare_function,
    module_ext::{BlockExt, ModuleExt},
    naga_expr,
    std_objects::std_objects_gen,
};

use super::{i64_instance_gen, I64Gen};

/// An implementation of i64s using a 2-vector of u32s
pub(crate) struct PolyfillI64;
impl I64Gen for PolyfillI64 {
    fn gen_ty(
        module: &mut naga::Module,
        _others: super::i64_instance_gen::TyRequirements,
    ) -> build::Result<super::i64_instance_gen::Ty> {
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

    fn gen_default(
        module: &mut naga::Module,
        others: super::i64_instance_gen::DefaultRequirements,
    ) -> build::Result<super::i64_instance_gen::Default> {
        Ok(super::make_64_bit_const_from_2vec32(
            others.ty,
            &mut module.constants,
            0,
        ))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _others: super::i64_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::i64_instance_gen::SizeBytes> {
        Ok(8)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        _others: super::i64_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::i64_instance_gen::MakeConst> {
        Ok(Arc::new(Box::new(|module, std_objects, value| {
            Ok(super::make_64_bit_const_from_2vec32(
                std_objects.i64.ty,
                module,
                value,
            ))
        })))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        others: super::i64_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::i64_instance_gen::ReadInput> {
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
        others: super::i64_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::i64_instance_gen::WriteOutput> {
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
        others: super::i64_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::i64_instance_gen::ReadMemory> {
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
        others: super::i64_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::i64_instance_gen::WriteMemory> {
        gen_write(
            module,
            others.word,
            others.ty,
            others.bindings.memory,
            "memory",
        )
    }

    fn gen_add(
        module: &mut naga::Module,
        others: super::i64_instance_gen::AddRequirements,
    ) -> build::Result<super::i64_instance_gen::Add> {
        let i64_ty = others.ty;
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i64_add(lhs: i64_ty, rhs: i64_ty) -> i64_ty
        };

        let lhs_high = naga_expr!(module, function_handle => lhs[const 0]);
        let lhs_low = naga_expr!(module, function_handle => lhs[const 1]);
        let rhs_high = naga_expr!(module, function_handle => rhs[const 0]);
        let rhs_low = naga_expr!(module, function_handle => rhs[const 1]);
        let carry_bit = naga_expr!(module, function_handle => if (lhs_low > (Constant(others.word_max) - rhs_low)) {U32(1)} else {U32(0)});
        let res_low = naga_expr!(module, function_handle => lhs_low + rhs_low);
        let res_high = naga_expr!(module, function_handle => lhs_high + rhs_high + carry_bit);
        let res = naga_expr!(module, function_handle => i64_ty(res_high, res_low));
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32) -> i64
fn gen_read(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    i64_ty: i64_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_i64_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> i64_ty
    };

    let input_ref = naga_expr!(module, function_handle => Global(buffer));

    let read_word1 = naga_expr!(module, function_handle => input_ref[word_address]);
    let read_word2 = naga_expr!(module, function_handle => input_ref[word_address + U32(1)]);
    let read_value =
        naga_expr!(module, function_handle => i64_ty((Load(read_word1)), (Load(read_word2))));
    module.fn_mut(function_handle).body.push_return(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: i64)
fn gen_write(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    i64_ty: i64_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_i64_to_{}", buffer_name);
    let (handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: i64_ty)
    };

    let output_ref = naga_expr!(module, handle => Global(buffer));

    let write_word_loc1 = naga_expr!(module, handle => output_ref[word_address]);
    let word1 = naga_expr!(module, handle => value[const 0] as Uint);
    let write_word_loc2 = naga_expr!(module, handle => output_ref[word_address + (U32(1))]);
    let word2 = naga_expr!(module, handle => value[const 1] as Uint);

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
