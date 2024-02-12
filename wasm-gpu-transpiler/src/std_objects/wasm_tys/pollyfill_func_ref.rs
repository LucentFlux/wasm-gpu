use crate::{build, std_objects::preamble_objects_gen};
use naga_ext::BlockContext;
use naga_ext::{declare_function, naga_expr, ConstantsExt, ExpressionsExt, TypesExt};

use super::{func_ref_instance_gen, FuncRefGen};

/// An implementation of FuncRefs using the GPU's native u32 type
pub(crate) struct PolyfillFuncRef;
impl FuncRefGen for PolyfillFuncRef {
    fn gen_ty(
        module: &mut naga::Module,
        _requirements: super::func_ref_instance_gen::TyRequirements,
    ) -> build::Result<super::func_ref_instance_gen::Ty> {
        Ok(module.types.insert_u32())
    }

    fn gen_default(
        module: &mut naga::Module,
        requirements: super::func_ref_instance_gen::DefaultRequirements,
    ) -> build::Result<super::func_ref_instance_gen::Default> {
        let expr = module.const_expressions.append_u32(u32::MAX);
        Ok(module.constants.append_anonymous(*requirements.ty, expr))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _requirements: super::func_ref_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::func_ref_instance_gen::SizeBytes> {
        Ok(4)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        _requirements: super::func_ref_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::func_ref_instance_gen::MakeConst> {
        Ok(Box::new(move |const_expressions, value| {
            let value = value.as_u32().unwrap_or(u32::MAX).into();
            let expr = const_expressions.append_u32(value);
            Ok(expr)
        }))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        requirements: super::func_ref_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::func_ref_instance_gen::ReadInput> {
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
        requirements: super::func_ref_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::func_ref_instance_gen::WriteOutput> {
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
        requirements: super::func_ref_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::func_ref_instance_gen::ReadMemory> {
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
        requirements: super::func_ref_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::func_ref_instance_gen::WriteMemory> {
        gen_write(
            module,
            requirements.preamble.word_ty,
            *requirements.ty,
            requirements.preamble.bindings.memory,
            "memory",
        )
    }
}

// fn<buffer>(word_address: u32) -> func_ref
fn gen_read(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    func_ref_ty: func_ref_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_func_ref_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> func_ref_ty
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let read_value = naga_expr!(&mut ctx => Load(Global(buffer)[word_address]));
    ctx.result(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: func_ref)
fn gen_write(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    func_ref_ty: func_ref_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_func_ref_to_{}", buffer_name);
    let (function_handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: func_ref_ty)
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let write_word_loc = naga_expr!(&mut ctx => Global(buffer)[word_address]);
    let word = naga_expr!(&mut ctx => value as Uint);
    ctx.store(write_word_loc, word);

    Ok(function_handle)
}
