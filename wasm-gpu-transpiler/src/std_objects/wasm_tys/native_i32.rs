use crate::{build, std_objects::preamble_objects_gen};
use naga_ext::BlockContext;
use naga_ext::{declare_function, naga_expr, ConstantsExt, ExpressionsExt, TypesExt};
use wasmtime_environ::Trap;

use super::{i32_instance_gen, I32Gen};

/// An implementation of i32s using the GPU's native i32 type
pub(crate) struct NativeI32;
impl I32Gen for NativeI32 {
    fn gen_ty(
        module: &mut naga::Module,
        _requirements: super::i32_instance_gen::TyRequirements,
    ) -> build::Result<super::i32_instance_gen::Ty> {
        Ok(module.types.insert_i32())
    }

    fn gen_default(
        module: &mut naga::Module,
        requirements: super::i32_instance_gen::DefaultRequirements,
    ) -> build::Result<super::i32_instance_gen::Default> {
        let expr = module.const_expressions.append_i32(0);
        Ok(module.constants.append_anonymous(*requirements.ty, expr))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _requirements: super::i32_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::i32_instance_gen::SizeBytes> {
        Ok(4)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        _requirements: super::i32_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::i32_instance_gen::MakeConst> {
        Ok(Box::new(|const_expressions, value| {
            Ok(const_expressions.append_i32(value))
        }))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        requirements: super::i32_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::i32_instance_gen::ReadInput> {
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
        requirements: super::i32_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::i32_instance_gen::WriteOutput> {
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
        requirements: super::i32_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::i32_instance_gen::ReadMemory> {
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
        requirements: super::i32_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::i32_instance_gen::WriteMemory> {
        gen_write(
            module,
            requirements.preamble.word_ty,
            *requirements.ty,
            requirements.preamble.bindings.memory,
            "memory",
        )
    }

    super::impl_integer_loads_and_stores! {i32_instance_gen, i32}

    fn gen_eqz(
        module: &mut naga::Module,
        requirements: i32_instance_gen::EqzRequirements,
    ) -> build::Result<i32_instance_gen::Eqz> {
        let (function_handle, value) = declare_function! {
            module => fn i32_eqz(value: *requirements.ty) -> requirements.preamble.wasm_bool.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let t = naga_expr!(ctx => Constant(requirements.preamble.wasm_bool.const_true));
        let f = naga_expr!(ctx => Constant(requirements.preamble.wasm_bool.const_false));
        let res = naga_expr!(ctx => if (value == I32(0)) {t} else {f});
        ctx.result(res);

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
        requirements: i32_instance_gen::ClzRequirements,
    ) -> build::Result<i32_instance_gen::Clz> {
        let (function_handle, value) = declare_function! {
          module => fn i32_clz(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = ctx.append_expr(naga::Expression::Math {
            fun: naga::MathFunction::FindMsb,
            arg: value,
            arg1: None,
            arg2: None,
            arg3: None,
        });
        let res = naga_expr!(ctx => I32(31) - res);
        // Check if last bit set
        let res = naga_expr!(ctx => if ((value & I32(-2147483648)) != I32(0)) {I32(0)} else {res});
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_ctz(
        module: &mut naga::Module,
        requirements: i32_instance_gen::CtzRequirements,
    ) -> build::Result<i32_instance_gen::Ctz> {
        let (function_handle, value) = declare_function! {
          module => fn i32_ctz(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = ctx.append_expr(naga::Expression::Math {
            fun: naga::MathFunction::FindLsb,
            arg: value,
            arg1: None,
            arg2: None,
            arg3: None,
        });
        let res = naga_expr!(ctx => if (value == I32(0)) {I32(32)} else {res});
        ctx.result(res);

        Ok(function_handle)
    }

    super::impl_native_unary_inner_math_fn! {i32_instance_gen, i32, popcnt; CountOneBits}

    fn gen_div_s(
        module: &mut naga::Module,
        requirements: i32_instance_gen::DivSRequirements,
    ) -> build::Result<i32_instance_gen::DivS> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn i32_div_s(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // Div by 0 test
        let is_0 = naga_expr!(ctx => rhs == I32(0));
        ctx.test(is_0).then(|mut ctx| {
            requirements.preamble.trap_values.emit_set_trap(
                &mut ctx,
                Trap::IntegerDivisionByZero,
                requirements.preamble.trap_state,
            );
        });

        // Overflow test
        let is_overflowing = naga_expr!(ctx => (lhs == I32(-2147483648)) & (rhs == I32(-1)));
        ctx.test(is_0).then(|mut ctx| {
            requirements.preamble.trap_values.emit_set_trap(
                &mut ctx,
                Trap::IntegerOverflow,
                requirements.preamble.trap_state,
            );
        });

        let res = naga_expr!(ctx => lhs/rhs);
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_div_u(
        module: &mut naga::Module,
        requirements: i32_instance_gen::DivURequirements,
    ) -> build::Result<i32_instance_gen::DivU> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn i32_div_u(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // Div by 0 test
        let is_0 = naga_expr!(ctx => rhs == I32(0));
        ctx.test(is_0).then(|mut ctx| {
            requirements.preamble.trap_values.emit_set_trap(
                &mut ctx,
                Trap::IntegerDivisionByZero,
                requirements.preamble.trap_state,
            );
        });

        let res = naga_expr!(ctx => ((lhs as Uint)/(rhs as Uint))as Sint);
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_rem_s(
        module: &mut naga::Module,
        requirements: i32_instance_gen::RemSRequirements,
    ) -> build::Result<i32_instance_gen::RemS> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn i32_rem_s(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // Div by 0 test
        let is_0 = naga_expr!(ctx => rhs == I32(0));
        ctx.test(is_0).then(|mut ctx| {
            requirements.preamble.trap_values.emit_set_trap(
                &mut ctx,
                Trap::IntegerDivisionByZero,
                requirements.preamble.trap_state,
            );
        });

        let res = naga_expr!(ctx => lhs%rhs);
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_rem_u(
        module: &mut naga::Module,
        requirements: i32_instance_gen::RemURequirements,
    ) -> build::Result<i32_instance_gen::RemU> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn i32_rem_u(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // Div by 0 test
        let is_0 = naga_expr!(ctx => rhs == I32(0));
        ctx.test(is_0).then(|mut ctx| {
            requirements.preamble.trap_values.emit_set_trap(
                &mut ctx,
                Trap::IntegerDivisionByZero,
                requirements.preamble.trap_state,
            );
        });

        let res = naga_expr!(ctx => ((lhs as Uint)%(rhs as Uint))as Sint);
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_rotl(
        module: &mut naga::Module,
        requirements: i32_instance_gen::RotlRequirements,
    ) -> build::Result<i32_instance_gen::Rotl> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i32_rotl(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let lhs = naga_expr!(ctx => lhs as Uint);
        let rhs = naga_expr!(ctx => rhs % U32(32)); // Between -31 and 31
        let rhs = naga_expr!(ctx => rhs + U32(32));
        let rhs = naga_expr!(ctx => rhs % U32(32)); // Between 0 and 31
        let res = naga_expr!(ctx => (lhs << rhs) | (lhs >> (U32(32) - rhs)));
        let res = naga_expr!(ctx => res as Sint);
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_rotr(
        module: &mut naga::Module,
        requirements: i32_instance_gen::RotrRequirements,
    ) -> build::Result<i32_instance_gen::Rotr> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i32_rotr(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let lhs = naga_expr!(ctx => lhs as Uint);
        let rhs = naga_expr!(ctx => rhs % U32(32)); // Between -31 and 31
        let rhs = naga_expr!(ctx => rhs + U32(32));
        let rhs = naga_expr!(ctx => rhs % U32(32)); // Between 0 and 31
        let res = naga_expr!(ctx => (lhs >> rhs) | (lhs << (U32(32) - rhs)));
        let res = naga_expr!(ctx => res as Sint);
        ctx.result(res);

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
        requirements: i32_instance_gen::Extend8SRequirements,
    ) -> build::Result<i32_instance_gen::Extend8S> {
        let (function_handle, value) = declare_function! {
            module => fn i32_extend_8_s(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = naga_expr!(ctx => (value << U32(24)) >> U32(24));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_extend_16_s(
        module: &mut naga::Module,
        requirements: i32_instance_gen::Extend16SRequirements,
    ) -> build::Result<i32_instance_gen::Extend16S> {
        let (function_handle, value) = declare_function! {
            module => fn i32_extend_16_s(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = naga_expr!(ctx => (value << U32(16)) >> U32(16));
        ctx.result(res);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32) -> i32
fn gen_read(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    i32_ty: i32_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_i32_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> i32_ty
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let read_word = naga_expr!(ctx => Global(buffer)[word_address]);
    let read_value = naga_expr!(ctx => Load(read_word) as Sint);
    ctx.result(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: i32)
fn gen_write(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    i32_ty: i32_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_i32_to_{}", buffer_name);
    let (function_handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: i32_ty)
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let write_word_loc = naga_expr!(ctx => Global(buffer)[word_address]);
    let word = naga_expr!(ctx => value as Uint);
    ctx.store(write_word_loc, word);

    Ok(function_handle)
}
