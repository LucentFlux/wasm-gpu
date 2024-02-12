use crate::build;
use crate::std_objects::preamble_objects_gen;
use crate::typed::V128;
use naga_ext::{declare_function, naga_expr, BlockContext, ConstantsExt, ExpressionsExt, TypesExt};

use super::{v128_instance_gen, V128Gen};

fn make_const_expr_impl(
    const_expressions: &mut naga::Arena<naga::Expression>,
    ty: naga::Handle<naga::Type>,
    value: V128,
) -> naga::Handle<naga::Expression> {
    let bytes = value.to_le_bytes();
    let components = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| {
            let word = u32::from_le_bytes(*bytes);
            let word = u32::try_from(word & 0xFFFFFFFF).expect("truncated word always fits in u32");
            const_expressions.append_u32(word)
        })
        .collect();
    const_expressions.append_compose(ty, components)
}

/// An implementation of v128s using a 4-vector of u32s. Calling this a Polyfill is a slight stretch
/// since a v128 is almost exactly an ivec4, but some things don't map perfectly so it's a polyfill.
pub(crate) struct PolyfillV128;
impl V128Gen for PolyfillV128 {
    fn gen_ty(
        module: &mut naga::Module,
        _requirements: super::v128_instance_gen::TyRequirements,
    ) -> build::Result<super::v128_instance_gen::Ty> {
        let ty = module.types.insert_anonymous(naga::TypeInner::Vector {
            size: naga::VectorSize::Quad,
            scalar: naga::Scalar::U32,
        });

        Ok(ty)
    }

    fn gen_default(
        module: &mut naga::Module,
        requirements: super::v128_instance_gen::DefaultRequirements,
    ) -> build::Result<super::v128_instance_gen::Default> {
        let init = make_const_expr_impl(
            &mut module.const_expressions,
            *requirements.ty,
            V128::from_bits(0),
        );
        Ok(module.constants.append_anonymous(*requirements.ty, init))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _requirements: super::v128_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::v128_instance_gen::SizeBytes> {
        Ok(16)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        requirements: super::v128_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::v128_instance_gen::MakeConst> {
        let ty = *requirements.ty;
        Ok(Box::new(move |const_expressions, value| {
            Ok(make_const_expr_impl(const_expressions, ty, value))
        }))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        requirements: super::v128_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::v128_instance_gen::ReadInput> {
        gen_read(
            module,
            requirements.preamble.word_ty,
            *requirements.ty,
            requirements.preamble.bindings.input,
            "input",
        )
    }

    fn gen_write_output(
        module: &mut naga::Module,
        requirements: super::v128_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::v128_instance_gen::WriteOutput> {
        gen_write(
            module,
            requirements.preamble.word_ty,
            *requirements.ty,
            requirements.preamble.bindings.output,
            "output",
        )
    }

    fn gen_read_memory(
        module: &mut naga::Module,
        requirements: super::v128_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::v128_instance_gen::ReadMemory> {
        gen_read(
            module,
            requirements.preamble.word_ty,
            *requirements.ty,
            requirements.preamble.bindings.memory,
            "memory",
        )
    }

    fn gen_write_memory(
        module: &mut naga::Module,
        requirements: super::v128_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::v128_instance_gen::WriteMemory> {
        gen_write(
            module,
            requirements.preamble.word_ty,
            *requirements.ty,
            requirements.preamble.bindings.memory,
            "memory",
        )
    }
}

// fn<buffer>(word_address: u32) -> v128
fn gen_read(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    v128_ty: v128_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_v128_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> v128_ty
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let input_ref = naga_expr!(&mut ctx => Global(buffer));

    let read_word1 = naga_expr!(&mut ctx => input_ref[word_address]);
    let read_word2 = naga_expr!(&mut ctx => input_ref[word_address + (U32(1))]);
    let read_word3 = naga_expr!(&mut ctx => input_ref[word_address + (U32(2))]);
    let read_word4 = naga_expr!(&mut ctx => input_ref[word_address + (U32(3))]);
    let read_value = naga_expr!(&mut ctx => v128_ty(
        (Load(read_word1)),
        (Load(read_word2)),
        (Load(read_word3)),
        (Load(read_word4))
    ));
    ctx.result(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: v128)
fn gen_write(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    v128_ty: v128_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_v128_to_{}", buffer_name);
    let (function_handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: v128_ty)
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let output_ref = naga_expr!(&mut ctx => Global(buffer));

    let write_word_loc1 = naga_expr!(&mut ctx => output_ref[word_address]);
    let word1 = naga_expr!(&mut ctx => value[const 0]);
    let write_word_loc2 = naga_expr!(&mut ctx => output_ref[word_address + (U32(1))]);
    let word2 = naga_expr!(&mut ctx => value[const 1]);
    let write_word_loc3 = naga_expr!(&mut ctx => output_ref[word_address + (U32(2))]);
    let word3 = naga_expr!(&mut ctx => value[const 2]);
    let write_word_loc4 = naga_expr!(&mut ctx => output_ref[word_address + (U32(3))]);
    let word4 = naga_expr!(&mut ctx => value[const 3]);

    ctx.store(write_word_loc1, word1);
    ctx.store(write_word_loc2, word2);
    ctx.store(write_word_loc3, word3);
    ctx.store(write_word_loc4, word4);

    Ok(function_handle)
}
