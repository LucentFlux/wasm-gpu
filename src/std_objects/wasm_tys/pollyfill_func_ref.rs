use std::sync::Arc;

use wasm_types::FuncRef;

use crate::{
    build, declare_function,
    module_ext::{BlockExt, ModuleExt},
    naga_expr,
    std_objects::std_objects_gen,
};

use super::{func_ref_instance_gen, FuncRefGen};

fn make_const_impl(
    constants: &mut naga::Arena<naga::Constant>,
    value: FuncRef,
) -> naga::Handle<naga::Constant> {
    constants.append(
        naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar {
                width: 4,
                value: naga::ScalarValue::Uint(value.as_u32().unwrap_or(u32::MAX).into()),
            },
        },
        naga::Span::UNDEFINED,
    )
}

/// An implementation of FuncRefs using the GPU's native u32 type
pub(crate) struct PolyfillFuncRef;
impl FuncRefGen for PolyfillFuncRef {
    fn gen_ty(
        module: &mut naga::Module,
        _others: super::func_ref_instance_gen::TyRequirements,
    ) -> build::Result<super::func_ref_instance_gen::Ty> {
        let naga_ty = naga::Type {
            name: Some("FuncRef".to_owned()),
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }

    fn gen_default(
        module: &mut naga::Module,
        _others: super::func_ref_instance_gen::DefaultRequirements,
    ) -> build::Result<super::func_ref_instance_gen::Default> {
        Ok(make_const_impl(&mut module.constants, FuncRef::none()))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _others: super::func_ref_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::func_ref_instance_gen::SizeBytes> {
        Ok(4)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        _others: super::func_ref_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::func_ref_instance_gen::MakeConst> {
        Ok(Arc::new(Box::new(|module, _, value| {
            Ok(make_const_impl(module, value))
        })))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        others: super::func_ref_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::func_ref_instance_gen::ReadInput> {
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
        others: super::func_ref_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::func_ref_instance_gen::WriteOutput> {
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
        others: super::func_ref_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::func_ref_instance_gen::ReadMemory> {
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
        others: super::func_ref_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::func_ref_instance_gen::WriteMemory> {
        gen_write(
            module,
            others.word,
            others.ty,
            others.bindings.memory,
            "memory",
        )
    }
}

// fn<buffer>(word_address: u32) -> func_ref
fn gen_read(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    func_ref_ty: func_ref_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_func_ref_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> func_ref_ty
    };

    let read_value = naga_expr!(module, function_handle => Load(Global(buffer)[word_address]));
    module.fn_mut(function_handle).body.push_return(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: func_ref)
fn gen_write(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    func_ref_ty: func_ref_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_func_ref_to_{}", buffer_name);
    let (function_handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: func_ref_ty)
    };

    let write_word_loc = naga_expr!(module, function_handle => Global(buffer)[word_address]);
    let word = naga_expr!(module, function_handle => value as Uint);
    module
        .fn_mut(function_handle)
        .body
        .push_store(write_word_loc, word);

    Ok(function_handle)
}
