use crate::{
    build,
    std_objects::{preamble_objects_gen, WasmBoolInstance},
};
use naga_ext::{declare_function, naga_expr, BlockContext, ConstantsExt, TypesExt};

use super::{i64_instance_gen, I64Gen};

fn gen_boolean_mono(
    module: &mut naga::Module,
    f64_ty: naga::Handle<naga::Type>,
    wasm_bool: &WasmBoolInstance,
    name: &str,
    make: impl FnOnce(
        &mut BlockContext<'_>,
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression>,
) -> build::Result<naga::Handle<naga::Function>> {
    let (function_handle, value) = declare_function! {
        module => fn {format!("i64_{}", name)}(value: f64_ty) -> wasm_bool.ty
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let t = naga_expr!(&mut ctx => Constant(wasm_bool.const_true));
    let f = naga_expr!(&mut ctx => Constant(wasm_bool.const_false));

    let value_high = naga_expr!(&mut ctx => value[const 0]);
    let value_low = naga_expr!(&mut ctx => value[const 1]);
    let cond = make(&mut ctx, value_high, value_low);
    let res = naga_expr!(&mut ctx => if (cond) {t} else {f});
    ctx.result(res);

    Ok(function_handle)
}

fn gen_boolean_binary(
    module: &mut naga::Module,
    f64_ty: naga::Handle<naga::Type>,
    wasm_bool: &WasmBoolInstance,
    name: &str,
    make: impl FnOnce(
        &mut BlockContext<'_>,
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression>,
) -> build::Result<naga::Handle<naga::Function>> {
    let (function_handle, lhs, rhs) = declare_function! {
        module => fn {format!("i64_{}", name)}(lhs: f64_ty, rhs: f64_ty) -> wasm_bool.ty
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let t = naga_expr!(&mut ctx => Constant(wasm_bool.const_true));
    let f = naga_expr!(&mut ctx => Constant(wasm_bool.const_false));

    let lhs_high = naga_expr!(&mut ctx => lhs[const 0]);
    let lhs_low = naga_expr!(&mut ctx => lhs[const 1]);
    let rhs_high = naga_expr!(&mut ctx => rhs[const 0]);
    let rhs_low = naga_expr!(&mut ctx => rhs[const 1]);
    let cond = make(&mut ctx, lhs_high, lhs_low, rhs_high, rhs_low);
    let res = naga_expr!(&mut ctx => if (cond) {t} else {f});
    ctx.result(res);

    Ok(function_handle)
}

/// An implementation of i64s using a 2-vector of u32s
pub(crate) struct PolyfillI64;
impl I64Gen for PolyfillI64 {
    fn gen_ty(
        module: &mut naga::Module,
        _requirements: super::i64_instance_gen::TyRequirements,
    ) -> build::Result<super::i64_instance_gen::Ty> {
        let ty = module.types.insert_anonymous(naga::TypeInner::Vector {
            size: naga::VectorSize::Bi,
            scalar: naga::Scalar::U32,
        });

        Ok(ty)
    }

    fn gen_default(
        module: &mut naga::Module,
        requirements: super::i64_instance_gen::DefaultRequirements,
    ) -> build::Result<super::i64_instance_gen::Default> {
        let init = super::make_64_bit_const_expr_from_2vec32(
            *requirements.ty,
            &mut module.const_expressions,
            0,
        );
        Ok(module.constants.append_anonymous(*requirements.ty, init))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _requirements: super::i64_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::i64_instance_gen::SizeBytes> {
        Ok(8)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        requirements: super::i64_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::i64_instance_gen::MakeConst> {
        let ty = *requirements.ty;
        Ok(Box::new(move |const_expressions, value| {
            Ok(super::make_64_bit_const_expr_from_2vec32(
                ty,
                const_expressions,
                value,
            ))
        }))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        requirements: super::i64_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::i64_instance_gen::ReadInput> {
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
        requirements: super::i64_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::i64_instance_gen::WriteOutput> {
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
        requirements: super::i64_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::i64_instance_gen::ReadMemory> {
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
        requirements: super::i64_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::i64_instance_gen::WriteMemory> {
        gen_write(
            module,
            requirements.preamble.word_ty,
            *requirements.ty,
            requirements.preamble.bindings.memory,
            "memory",
        )
    }

    fn gen_add(
        module: &mut naga::Module,
        requirements: super::i64_instance_gen::AddRequirements,
    ) -> build::Result<super::i64_instance_gen::Add> {
        let i64_ty = *requirements.ty;
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i64_add(lhs: i64_ty, rhs: i64_ty) -> i64_ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let lhs_high = naga_expr!(&mut ctx => lhs[const 0]);
        let lhs_low = naga_expr!(&mut ctx => lhs[const 1]);
        let rhs_high = naga_expr!(&mut ctx => rhs[const 0]);
        let rhs_low = naga_expr!(&mut ctx => rhs[const 1]);
        let carry_bit = naga_expr!(&mut ctx => if (lhs_low > (Constant(requirements.preamble.word_max) - rhs_low)) {U32(1)} else {U32(0)});
        let res_low = naga_expr!(&mut ctx => lhs_low + rhs_low);
        let res_high = naga_expr!(&mut ctx => lhs_high + rhs_high + carry_bit);
        let res = naga_expr!(&mut ctx => i64_ty(res_high, res_low));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_sub(
        module: &mut naga::Module,
        requirements: super::i64_instance_gen::SubRequirements,
    ) -> build::Result<super::i64_instance_gen::Sub> {
        let i64_ty = *requirements.ty;
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i64_sub(lhs: i64_ty, rhs: i64_ty) -> i64_ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let lhs_high = naga_expr!(&mut ctx => lhs[const 0]);
        let lhs_low = naga_expr!(&mut ctx => lhs[const 1]);
        let rhs_high = naga_expr!(&mut ctx => rhs[const 0]);
        let rhs_low = naga_expr!(&mut ctx => rhs[const 1]);
        let carry_condition = naga_expr!(&mut ctx => lhs_low < rhs_low);
        let res_low = naga_expr!(&mut ctx => if (carry_condition) {
            (Constant(requirements.preamble.word_max) - rhs_low) + lhs_low + U32(1)
        } else {
            lhs_low - rhs_low
        });
        let res_high = naga_expr!(&mut ctx => lhs_high - rhs_high - if (carry_condition) {U32(1)} else {U32(0)});
        let res = naga_expr!(&mut ctx => i64_ty(res_high, res_low));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_mul(
        module: &mut naga::Module,
        requirements: super::i64_instance_gen::MulRequirements,
    ) -> build::Result<super::i64_instance_gen::Mul> {
        let i64_ty = *requirements.ty;
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i64_mul(lhs: i64_ty, rhs: i64_ty) -> i64_ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // TODO

        ctx.result(lhs);

        Ok(function_handle)
    }

    super::impl_bitwise_2vec32_numeric_ops! {i64_instance_gen, i64}

    fn gen_eqz(
        module: &mut naga::Module,
        requirements: i64_instance_gen::EqzRequirements,
    ) -> build::Result<i64_instance_gen::Eqz> {
        gen_boolean_mono(
            module,
            *requirements.ty,
            &requirements.preamble.wasm_bool,
            "eqz",
            |ctx, high, low| {
                naga_expr!(ctx =>
                    (high == U32(0)) & (low == U32(0))
                )
            },
        )
    }

    fn gen_lt_s(
        module: &mut naga::Module,
        requirements: i64_instance_gen::LtSRequirements,
    ) -> build::Result<i64_instance_gen::LtS> {
        gen_boolean_binary(
            module,
            *requirements.ty,
            &requirements.preamble.wasm_bool,
            "lt_s",
            |ctx, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(ctx =>
                    ((lhs_high as Sint) < (rhs_high as Sint)) | ((lhs_high == rhs_high) & (lhs_low < rhs_low))
                )
            },
        )
    }

    fn gen_le_s(
        module: &mut naga::Module,
        requirements: i64_instance_gen::LeSRequirements,
    ) -> build::Result<i64_instance_gen::LeS> {
        gen_boolean_binary(
            module,
            *requirements.ty,
            &requirements.preamble.wasm_bool,
            "le_s",
            |ctx, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(ctx =>
                    ((lhs_high as Sint) < (rhs_high as Sint)) | ((lhs_high == rhs_high) & (lhs_low <= rhs_low))
                )
            },
        )
    }

    fn gen_gt_s(
        module: &mut naga::Module,
        requirements: i64_instance_gen::GtSRequirements,
    ) -> build::Result<i64_instance_gen::GtS> {
        gen_boolean_binary(
            module,
            *requirements.ty,
            &requirements.preamble.wasm_bool,
            "gt_s",
            |ctx, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(ctx =>
                    ((lhs_high as Sint) > (rhs_high as Sint)) | ((lhs_high == rhs_high) & (lhs_low > rhs_low))
                )
            },
        )
    }

    fn gen_ge_s(
        module: &mut naga::Module,
        requirements: i64_instance_gen::GeSRequirements,
    ) -> build::Result<i64_instance_gen::GeS> {
        gen_boolean_binary(
            module,
            *requirements.ty,
            &requirements.preamble.wasm_bool,
            "ge_s",
            |ctx, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(ctx =>
                    ((lhs_high as Sint) > (rhs_high as Sint)) | ((lhs_high == rhs_high) & (lhs_low >= rhs_low))
                )
            },
        )
    }

    fn gen_lt_u(
        module: &mut naga::Module,
        requirements: i64_instance_gen::LtURequirements,
    ) -> build::Result<i64_instance_gen::LtU> {
        gen_boolean_binary(
            module,
            *requirements.ty,
            &requirements.preamble.wasm_bool,
            "lt_u",
            |ctx, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(ctx =>
                    (lhs_high < rhs_high) | ((lhs_high == rhs_high) & (lhs_low < rhs_low))
                )
            },
        )
    }

    fn gen_le_u(
        module: &mut naga::Module,
        requirements: i64_instance_gen::LeURequirements,
    ) -> build::Result<i64_instance_gen::LeU> {
        gen_boolean_binary(
            module,
            *requirements.ty,
            &requirements.preamble.wasm_bool,
            "le_u",
            |ctx, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(ctx =>
                    (lhs_high < rhs_high) | ((lhs_high == rhs_high) & (lhs_low <= rhs_low))
                )
            },
        )
    }

    fn gen_gt_u(
        module: &mut naga::Module,
        requirements: i64_instance_gen::GtURequirements,
    ) -> build::Result<i64_instance_gen::GtU> {
        gen_boolean_binary(
            module,
            *requirements.ty,
            &requirements.preamble.wasm_bool,
            "gt_u",
            |ctx, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(ctx =>
                    (lhs_high > rhs_high) | ((lhs_high == rhs_high) & (lhs_low > rhs_low))
                )
            },
        )
    }

    fn gen_ge_u(
        module: &mut naga::Module,
        requirements: i64_instance_gen::GeURequirements,
    ) -> build::Result<i64_instance_gen::GeU> {
        gen_boolean_binary(
            module,
            *requirements.ty,
            &requirements.preamble.wasm_bool,
            "ge_u",
            |ctx, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(ctx =>
                    (lhs_high > rhs_high) | ((lhs_high == rhs_high) & (lhs_low >= rhs_low))
                )
            },
        )
    }

    super::impl_load_and_store! {i64_instance_gen, i64}

    super::impl_integer_loads_and_stores! {i64_instance_gen, i64}

    super::impl_dud_integer_load! {i64_instance_gen, i64, load_32_u}
    super::impl_dud_integer_load! {i64_instance_gen, i64, load_32_s}
    super::impl_dud_integer_store! {i64_instance_gen, i64, store_32}

    //super::impl_integer_atomic_loads_and_stores! {i64_instance_gen, i64}

    /*super::impl_dud_integer_load! {i64_instance_gen, i64, atomic_load_32_u}
    super::impl_dud_integer_store! {i64_instance_gen, i64, atomic_store_32}
    super::impl_dud_integer_rmw! {i64_instance_gen, i64, atomic_rmw_32_add_u}
    super::impl_dud_integer_rmw! {i64_instance_gen, i64, atomic_rmw_32_sub_u}
    super::impl_dud_integer_rmw! {i64_instance_gen, i64, atomic_rmw_32_and_u}
    super::impl_dud_integer_rmw! {i64_instance_gen, i64, atomic_rmw_32_or_u}
    super::impl_dud_integer_rmw! {i64_instance_gen, i64, atomic_rmw_32_xor_u}
    super::impl_dud_integer_rmw! {i64_instance_gen, i64, atomic_rmw_32_xchg_u}
    super::impl_dud_integer_rmw! {i64_instance_gen, i64, atomic_rmw_32_cmpxchg_u}*/

    super::impl_dud_inner_binexp! {i64_instance_gen, i64, clz }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, ctz }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, div_s }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, div_u }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, rem_s }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, rem_u }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, rotl }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, rotr }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, popcnt }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, and }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, or }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, xor }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, shl }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, shr_s }
    super::impl_dud_inner_binexp! {i64_instance_gen, i64, shr_u }

    fn gen_extend_8_s(
        module: &mut naga::Module,
        requirements: i64_instance_gen::Extend8SRequirements,
    ) -> build::Result<i64_instance_gen::Extend8S> {
        let (function_handle, value) = declare_function! {
            module => fn i64_extend_8_s(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let low = naga_expr!(&mut ctx => value[const 1] as Sint);
        let high = naga_expr!(&mut ctx => (low << U32(31)) >> U32(31));
        let low = naga_expr!(&mut ctx => (low << U32(24)) >> U32(24));
        let res = naga_expr!(&mut ctx => (*requirements.ty)(high as Uint, low as Uint));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_extend_16_s(
        module: &mut naga::Module,
        requirements: i64_instance_gen::Extend16SRequirements,
    ) -> build::Result<i64_instance_gen::Extend16S> {
        let (function_handle, value) = declare_function! {
            module => fn i64_extend_16_s(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let low = naga_expr!(&mut ctx => value[const 1] as Sint);
        let high = naga_expr!(&mut ctx => (low << U32(31)) >> U32(31));
        let low = naga_expr!(&mut ctx => (low << U32(16)) >> U32(16));
        let res = naga_expr!(&mut ctx => (*requirements.ty)(high as Uint, low as Uint));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_extend_32_s(
        module: &mut naga::Module,
        requirements: i64_instance_gen::Extend32SRequirements,
    ) -> build::Result<i64_instance_gen::Extend32S> {
        let (function_handle, value) = declare_function! {
            module => fn i64_extend_16_s(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let low = naga_expr!(&mut ctx => value[const 1]);
        let high = naga_expr!(&mut ctx => ((low as Sint) << U32(31)) >> U32(31));
        let res = naga_expr!(&mut ctx => (*requirements.ty)(high as Uint, low));
        ctx.result(res);

        Ok(function_handle)
    }
}

// fn<buffer>(word_address: u32) -> i64
fn gen_read(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    i64_ty: i64_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_i64_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> i64_ty
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let input_ref = naga_expr!(&mut ctx => Global(buffer));

    let read_word1 = naga_expr!(&mut ctx => input_ref[word_address]);
    let read_word2 = naga_expr!(&mut ctx => input_ref[word_address + U32(1)]);
    let read_value = naga_expr!(&mut ctx => i64_ty((Load(read_word1)), (Load(read_word2))));
    ctx.result(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: i64)
fn gen_write(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    i64_ty: i64_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_i64_to_{}", buffer_name);
    let (function_handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: i64_ty)
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let output_ref = naga_expr!(&mut ctx => Global(buffer));

    let write_word_loc1 = naga_expr!(&mut ctx => output_ref[word_address]);
    let word1 = naga_expr!(&mut ctx => value[const 0] as Uint);
    let write_word_loc2 = naga_expr!(&mut ctx => output_ref[word_address + (U32(1))]);
    let word2 = naga_expr!(&mut ctx => value[const 1] as Uint);

    ctx.store(write_word_loc1, word1);
    ctx.store(write_word_loc2, word2);

    Ok(function_handle)
}
