use std::sync::Arc;

use crate::{build, std_objects::std_objects_gen};
use naga_ext::{declare_function, naga_expr, BlockExt, ExpressionsExt, LocalsExt, ModuleExt};
use wasmtime_environ::Trap;

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

    //super::impl_integer_atomic_loads_and_stores! {i32_instance_gen, i32}

    fn gen_clz(
        module: &mut naga::Module,
        others: i32_instance_gen::ClzRequirements,
    ) -> build::Result<i32_instance_gen::Clz> {
        let (function_handle, value) = declare_function! {
          module => fn i32_clz(value:others.ty) -> others.ty
        };
        let res = module.fn_mut(function_handle).expressions.append(
            naga::Expression::Math {
                fun: naga::MathFunction::FindMsb,
                arg: value,
                arg1: None,
                arg2: None,
                arg3: None,
            },
            naga::Span::UNDEFINED,
        );
        module.fn_mut(function_handle).body.push_emit(res);
        let res = naga_expr!(module, function_handle => I32(31) - res);
        // Check if last bit set
        let res = naga_expr!(module, function_handle => if ((value & I32(-2147483648)) != I32(0)) {I32(0)} else {res});
        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_ctz(
        module: &mut naga::Module,
        others: i32_instance_gen::CtzRequirements,
    ) -> build::Result<i32_instance_gen::Ctz> {
        let (function_handle, value) = declare_function! {
          module => fn i32_ctz(value:others.ty) -> others.ty
        };
        let res = module.fn_mut(function_handle).expressions.append(
            naga::Expression::Math {
                fun: naga::MathFunction::FindLsb,
                arg: value,
                arg1: None,
                arg2: None,
                arg3: None,
            },
            naga::Span::UNDEFINED,
        );
        module.fn_mut(function_handle).body.push_emit(res);
        let res = naga_expr!(module, function_handle => if (value == I32(0)) {I32(32)} else {res});
        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    super::impl_native_unary_inner_math_fn! {i32_instance_gen, i32, popcnt; CountOneBits}

    fn gen_div_s(
        module: &mut naga::Module,
        others: i32_instance_gen::DivSRequirements,
    ) -> build::Result<i32_instance_gen::DivS> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn i32_div_s(lhs:others.ty,rhs:others.ty)->others.ty
        };

        // Div by 0 test
        let is_0 = naga_expr!(module, function_handle => rhs == I32(0));
        let mut if_0 = naga::Block::default();
        others.trap_values.emit_set_trap(
            Trap::IntegerDivisionByZero,
            others.trap_state,
            &mut (&mut *module, function_handle, &mut if_0),
        );
        module
            .fn_mut(function_handle)
            .body
            .push_if(is_0, if_0, naga::Block::default());

        // Overflow test
        let is_overflowing =
            naga_expr!(module, function_handle => (lhs == I32(-2147483648)) & (rhs == I32(-1)));
        let mut if_overflowing = naga::Block::default();
        others.trap_values.emit_set_trap(
            Trap::IntegerOverflow,
            others.trap_state,
            &mut (&mut *module, function_handle, &mut if_overflowing),
        );
        module.fn_mut(function_handle).body.push_if(
            is_overflowing,
            if_overflowing,
            naga::Block::default(),
        );

        let res = naga_expr!(module, function_handle => lhs/rhs);
        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_div_u(
        module: &mut naga::Module,
        others: i32_instance_gen::DivURequirements,
    ) -> build::Result<i32_instance_gen::DivU> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn i32_div_u(lhs:others.ty,rhs:others.ty)->others.ty
        };

        // Div by 0 test
        let is_0 = naga_expr!(module, function_handle => rhs == I32(0));
        let mut if_0 = naga::Block::default();
        others.trap_values.emit_set_trap(
            Trap::IntegerDivisionByZero,
            others.trap_state,
            &mut (&mut *module, function_handle, &mut if_0),
        );
        module
            .fn_mut(function_handle)
            .body
            .push_if(is_0, if_0, naga::Block::default());

        let res = naga_expr!(module, function_handle => ((lhs as Uint)/(rhs as Uint))as Sint);
        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_rem_s(
        module: &mut naga::Module,
        others: i32_instance_gen::RemSRequirements,
    ) -> build::Result<i32_instance_gen::RemS> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn i32_rem_s(lhs:others.ty,rhs:others.ty)->others.ty
        };

        // Div by 0 test
        let is_0 = naga_expr!(module, function_handle => rhs == I32(0));
        let mut if_0 = naga::Block::default();
        others.trap_values.emit_set_trap(
            Trap::IntegerDivisionByZero,
            others.trap_state,
            &mut (&mut *module, function_handle, &mut if_0),
        );
        module
            .fn_mut(function_handle)
            .body
            .push_if(is_0, if_0, naga::Block::default());

        let res = naga_expr!(module,function_handle => lhs%rhs);
        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_rem_u(
        module: &mut naga::Module,
        others: i32_instance_gen::RemURequirements,
    ) -> build::Result<i32_instance_gen::RemU> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn i32_rem_u(lhs:others.ty,rhs:others.ty)->others.ty
        };

        let trap_location = naga_expr!(module, function_handle => Global(others.trap_state));

        // Div by 0 test
        let is_0 = naga_expr!(module, function_handle => rhs == I32(0));
        let trap_code = others.trap_values.get(Trap::IntegerDivisionByZero);
        let trap_code = naga_expr!(module, function_handle => Constant(trap_code));
        let mut if_0 = naga::Block::default();
        if_0.push_store(trap_location, trap_code);
        module
            .fn_mut(function_handle)
            .body
            .push_if(is_0, if_0, naga::Block::default());

        let res = naga_expr!(module,function_handle => ((lhs as Uint)%(rhs as Uint))as Sint);
        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_rotl(
        module: &mut naga::Module,
        others: i32_instance_gen::RotlRequirements,
    ) -> build::Result<i32_instance_gen::Rotl> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i32_rotl(lhs: others.ty, rhs: others.ty) -> others.ty
        };

        let lhs = naga_expr!(module, function_handle => lhs as Uint);
        let rhs = naga_expr!(module, function_handle => rhs % U32(32)); // Between -31 and 31
        let rhs = naga_expr!(module, function_handle => rhs + U32(32));
        let rhs = naga_expr!(module, function_handle => rhs % U32(32)); // Between 0 and 31
        let res = naga_expr!(module, function_handle => (lhs << rhs) | (lhs >> (U32(32) - rhs)));
        let res = naga_expr!(module, function_handle => res as Sint);
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }

    fn gen_rotr(
        module: &mut naga::Module,
        others: i32_instance_gen::RotrRequirements,
    ) -> build::Result<i32_instance_gen::Rotr> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i32_rotr(lhs: others.ty, rhs: others.ty) -> others.ty
        };

        let lhs = naga_expr!(module, function_handle => lhs as Uint);
        let rhs = naga_expr!(module, function_handle => rhs % U32(32)); // Between -31 and 31
        let rhs = naga_expr!(module, function_handle => rhs + U32(32));
        let rhs = naga_expr!(module, function_handle => rhs % U32(32)); // Between 0 and 31
        let res = naga_expr!(module, function_handle => (lhs >> rhs) | (lhs << (U32(32) - rhs)));
        let res = naga_expr!(module, function_handle => res as Sint);
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }

    super::impl_native_inner_binexp!(i32_instance_gen, i32, and; &);
    super::impl_native_inner_binexp!(i32_instance_gen, i32, or; |);
    super::impl_native_inner_binexp!(i32_instance_gen, i32, xor; ^);
    super::impl_native_inner_binexp!(i32_instance_gen, i32, shl; <<);

    super::impl_native_inner_binexp!(i32_instance_gen, i32, shr_s; >>);
    super::impl_native_unsigned_inner_binexp!(i32_instance_gen, i32, shr_u; >>);

    fn gen_extend_8_s(
        module: &mut naga::Module,
        others: i32_instance_gen::Extend8SRequirements,
    ) -> build::Result<i32_instance_gen::Extend8S> {
        let (function_handle, value) = declare_function! {
            module => fn i32_extend_8_s(value: others.ty) -> others.ty
        };

        let res = naga_expr!(module, function_handle => (value << U32(24)) >> U32(24));
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }

    fn gen_extend_16_s(
        module: &mut naga::Module,
        others: i32_instance_gen::Extend16SRequirements,
    ) -> build::Result<i32_instance_gen::Extend16S> {
        let (function_handle, value) = declare_function! {
            module => fn i32_extend_16_s(value: others.ty) -> others.ty
        };

        let res = naga_expr!(module, function_handle => (value << U32(16)) >> U32(16));
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }
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
