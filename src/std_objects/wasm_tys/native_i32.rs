use std::sync::Arc;

use crate::{build, std_objects::std_objects_gen};
use naga_ext::{declare_function, naga_expr, BlockExt, ExpressionsExt, LocalsExt, ModuleExt};

use super::{i32_instance_gen, I32Gen};

fn make_const_impl(
    constants: &mut naga::Arena<naga::Constant>,
    value: i32,
) -> naga::Handle<naga::Constant> {
    constants.append(
        naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar {
                width: 4,
                value: naga::ScalarValue::Sint(value.into()),
            },
        },
        naga::Span::UNDEFINED,
    )
}

/// An implementation of i32s using the GPU's native i32 type
pub(crate) struct NativeI32;
impl I32Gen for NativeI32 {
    fn gen_ty(
        module: &mut naga::Module,
        _others: super::i32_instance_gen::TyRequirements,
    ) -> build::Result<super::i32_instance_gen::Ty> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Sint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }

    fn gen_default(
        module: &mut naga::Module,
        _others: super::i32_instance_gen::DefaultRequirements,
    ) -> build::Result<super::i32_instance_gen::Default> {
        Ok(make_const_impl(&mut module.constants, 0))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _others: super::i32_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::i32_instance_gen::SizeBytes> {
        Ok(4)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        _others: super::i32_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::i32_instance_gen::MakeConst> {
        Ok(Arc::new(Box::new(|module, _, value| {
            Ok(make_const_impl(module, value))
        })))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        others: super::i32_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::i32_instance_gen::ReadInput> {
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
        others: super::i32_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::i32_instance_gen::WriteOutput> {
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
        others: super::i32_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::i32_instance_gen::ReadMemory> {
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
        others: super::i32_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::i32_instance_gen::WriteMemory> {
        gen_write(
            module,
            others.word,
            others.ty,
            others.bindings.memory,
            "memory",
        )
    }

    super::impl_integer_loads_and_stores! {i32_instance_gen, i32}

    fn gen_eqz(
        module: &mut naga::Module,
        others: i32_instance_gen::EqzRequirements,
    ) -> build::Result<i32_instance_gen::Eqz> {
        let (function_handle, value) = declare_function! {
            module => fn i32_eqz(value: others.ty) -> others.wasm_bool.ty
        };

        let t = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_true));
        let f = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_false));
        let res = naga_expr!(module, function_handle => if (value == I32(0)) {t} else {f});
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }

    super::impl_native_ops! {i32_instance_gen, i32}

    super::impl_native_bool_binexp! {i32_instance_gen, i32, gt_s; >}
    super::impl_native_bool_binexp! {i32_instance_gen, i32, ge_s; >=}
    super::impl_native_bool_binexp! {i32_instance_gen, i32, lt_s; <}
    super::impl_native_bool_binexp! {i32_instance_gen, i32, le_s; <=}

    super::impl_native_unsigned_bool_binexp! {i32_instance_gen, i32, gt_u; >}
    super::impl_native_unsigned_bool_binexp! {i32_instance_gen, i32, ge_u; >=}
    super::impl_native_unsigned_bool_binexp! {i32_instance_gen, i32, lt_u; <}
    super::impl_native_unsigned_bool_binexp! {i32_instance_gen, i32, le_u; <=}

    super::impl_load_and_store! {i32_instance_gen, i32}

    super::impl_integer_atomic_loads_and_stores! {i32_instance_gen, i32}
}

// fn<buffer>(word_address: u32) -> i32
fn gen_read(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    i32_ty: i32_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_i32_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> i32_ty
    };

    let read_word = naga_expr!(module, function_handle => Global(buffer)[word_address]);
    let read_value = naga_expr!(module, function_handle => Load(read_word) as Sint);
    module.fn_mut(function_handle).body.push_return(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: i32)
fn gen_write(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    i32_ty: i32_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_i32_to_{}", buffer_name);
    let (function_handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: i32_ty)
    };

    let write_word_loc = naga_expr!(module, function_handle => Global(buffer)[word_address]);
    let word = naga_expr!(module, function_handle => value as Uint);
    module
        .fn_mut(function_handle)
        .body
        .push_store(write_word_loc, word);

    Ok(function_handle)
}
