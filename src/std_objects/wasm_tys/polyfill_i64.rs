use std::sync::Arc;

use crate::{
    build,
    std_objects::{std_objects_gen, WasmBoolInstance},
};
use naga_ext::{declare_function, naga_expr, BlockExt, ExpressionsExt, LocalsExt, ModuleExt};

use super::{i64_instance_gen, I64Gen};

fn gen_boolean_mono(
    module: &mut naga::Module,
    f64_ty: naga::Handle<naga::Type>,
    wasm_bool: WasmBoolInstance,
    name: &str,
    make: impl FnOnce(
        &mut naga::Module,
        naga::Handle<naga::Function>,
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression>,
) -> build::Result<naga::Handle<naga::Function>> {
    let (function_handle, value) = declare_function! {
        module => fn {format!("i64_{}", name)}(value: f64_ty) -> wasm_bool.ty
    };

    let t = naga_expr!(module, function_handle => Constant(wasm_bool.const_true));
    let f = naga_expr!(module, function_handle => Constant(wasm_bool.const_false));

    let value_high = naga_expr!(module, function_handle => value[const 0]);
    let value_low = naga_expr!(module, function_handle => value[const 1]);
    let cond = make(module, function_handle, value_high, value_low);
    let res = naga_expr!(module, function_handle => if (cond) {t} else {f});
    module.fn_mut(function_handle).body.push_return(res);

    Ok(function_handle)
}

fn gen_boolean_binary(
    module: &mut naga::Module,
    f64_ty: naga::Handle<naga::Type>,
    wasm_bool: WasmBoolInstance,
    name: &str,
    make: impl FnOnce(
        &mut naga::Module,
        naga::Handle<naga::Function>,
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Expression>,
        naga::Handle<naga::Expression>,
    ) -> naga::Handle<naga::Expression>,
) -> build::Result<naga::Handle<naga::Function>> {
    let (function_handle, lhs, rhs) = declare_function! {
        module => fn {format!("i64_{}", name)}(lhs: f64_ty, rhs: f64_ty) -> wasm_bool.ty
    };

    let t = naga_expr!(module, function_handle => Constant(wasm_bool.const_true));
    let f = naga_expr!(module, function_handle => Constant(wasm_bool.const_false));

    let lhs_high = naga_expr!(module, function_handle => lhs[const 0]);
    let lhs_low = naga_expr!(module, function_handle => lhs[const 1]);
    let rhs_high = naga_expr!(module, function_handle => rhs[const 0]);
    let rhs_low = naga_expr!(module, function_handle => rhs[const 1]);
    let cond = make(
        module,
        function_handle,
        lhs_high,
        lhs_low,
        rhs_high,
        rhs_low,
    );
    let res = naga_expr!(module, function_handle => if (cond) {t} else {f});
    module.fn_mut(function_handle).body.push_return(res);

    Ok(function_handle)
}

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

    fn gen_sub(
        module: &mut naga::Module,
        others: super::i64_instance_gen::SubRequirements,
    ) -> build::Result<super::i64_instance_gen::Sub> {
        let i64_ty = others.ty;
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i64_sub(lhs: i64_ty, rhs: i64_ty) -> i64_ty
        };

        let lhs_high = naga_expr!(module, function_handle => lhs[const 0]);
        let lhs_low = naga_expr!(module, function_handle => lhs[const 1]);
        let rhs_high = naga_expr!(module, function_handle => rhs[const 0]);
        let rhs_low = naga_expr!(module, function_handle => rhs[const 1]);
        let carry_condition = naga_expr!(module, function_handle => lhs_low < rhs_low);
        let res_low = naga_expr!(module, function_handle => if (carry_condition) {
            (Constant(others.word_max) - rhs_low) + lhs_low + U32(1)
        } else {
            lhs_low - rhs_low
        });
        let res_high = naga_expr!(module, function_handle => lhs_high - rhs_high - if (carry_condition) {U32(1)} else {U32(0)});
        let res = naga_expr!(module, function_handle => i64_ty(res_high, res_low));
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }

    fn gen_mul(
        module: &mut naga::Module,
        others: super::i64_instance_gen::MulRequirements,
    ) -> build::Result<super::i64_instance_gen::Mul> {
        let i64_ty = others.ty;
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn i64_mul(lhs: i64_ty, rhs: i64_ty) -> i64_ty
        };

        // TODO

        module.fn_mut(function_handle).body.push_return(lhs);

        Ok(function_handle)
    }

    super::impl_bitwise_2vec32_numeric_ops! {i64_instance_gen, i64}

    fn gen_eqz(
        module: &mut naga::Module,
        others: i64_instance_gen::EqzRequirements,
    ) -> build::Result<i64_instance_gen::Eqz> {
        gen_boolean_mono(
            module,
            others.ty,
            others.wasm_bool,
            "eqz",
            |module, handle, high, low| {
                naga_expr!(module, handle =>
                    (high == U32(0)) & (low == U32(0))
                )
            },
        )
    }

    fn gen_lt_s(
        module: &mut naga::Module,
        others: i64_instance_gen::LtSRequirements,
    ) -> build::Result<i64_instance_gen::LtS> {
        gen_boolean_binary(
            module,
            others.ty,
            others.wasm_bool,
            "lt_s",
            |module, handle, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(module, handle =>
                    ((lhs_high as Sint) < (rhs_high as Sint)) | ((lhs_high == rhs_high) & (lhs_low < rhs_low))
                )
            },
        )
    }

    fn gen_le_s(
        module: &mut naga::Module,
        others: i64_instance_gen::LeSRequirements,
    ) -> build::Result<i64_instance_gen::LeS> {
        gen_boolean_binary(
            module,
            others.ty,
            others.wasm_bool,
            "le_s",
            |module, handle, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(module, handle =>
                    ((lhs_high as Sint) < (rhs_high as Sint)) | ((lhs_high == rhs_high) & (lhs_low <= rhs_low))
                )
            },
        )
    }

    fn gen_gt_s(
        module: &mut naga::Module,
        others: i64_instance_gen::GtSRequirements,
    ) -> build::Result<i64_instance_gen::GtS> {
        gen_boolean_binary(
            module,
            others.ty,
            others.wasm_bool,
            "gt_s",
            |module, handle, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(module, handle =>
                    ((lhs_high as Sint) > (rhs_high as Sint)) | ((lhs_high == rhs_high) & (lhs_low > rhs_low))
                )
            },
        )
    }

    fn gen_ge_s(
        module: &mut naga::Module,
        others: i64_instance_gen::GeSRequirements,
    ) -> build::Result<i64_instance_gen::GeS> {
        gen_boolean_binary(
            module,
            others.ty,
            others.wasm_bool,
            "ge_s",
            |module, handle, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(module, handle =>
                    ((lhs_high as Sint) > (rhs_high as Sint)) | ((lhs_high == rhs_high) & (lhs_low >= rhs_low))
                )
            },
        )
    }

    fn gen_lt_u(
        module: &mut naga::Module,
        others: i64_instance_gen::LtURequirements,
    ) -> build::Result<i64_instance_gen::LtU> {
        gen_boolean_binary(
            module,
            others.ty,
            others.wasm_bool,
            "lt_u",
            |module, handle, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(module, handle =>
                    (lhs_high < rhs_high) | ((lhs_high == rhs_high) & (lhs_low < rhs_low))
                )
            },
        )
    }

    fn gen_le_u(
        module: &mut naga::Module,
        others: i64_instance_gen::LeURequirements,
    ) -> build::Result<i64_instance_gen::LeU> {
        gen_boolean_binary(
            module,
            others.ty,
            others.wasm_bool,
            "le_u",
            |module, handle, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(module, handle =>
                    (lhs_high < rhs_high) | ((lhs_high == rhs_high) & (lhs_low <= rhs_low))
                )
            },
        )
    }

    fn gen_gt_u(
        module: &mut naga::Module,
        others: i64_instance_gen::GtURequirements,
    ) -> build::Result<i64_instance_gen::GtU> {
        gen_boolean_binary(
            module,
            others.ty,
            others.wasm_bool,
            "gt_u",
            |module, handle, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(module, handle =>
                    (lhs_high > rhs_high) | ((lhs_high == rhs_high) & (lhs_low > rhs_low))
                )
            },
        )
    }

    fn gen_ge_u(
        module: &mut naga::Module,
        others: i64_instance_gen::GeURequirements,
    ) -> build::Result<i64_instance_gen::GeU> {
        gen_boolean_binary(
            module,
            others.ty,
            others.wasm_bool,
            "ge_u",
            |module, handle, lhs_high, lhs_low, rhs_high, rhs_low| {
                naga_expr!(module, handle =>
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
        others: i64_instance_gen::Extend8SRequirements,
    ) -> build::Result<i64_instance_gen::Extend8S> {
        let (function_handle, value) = declare_function! {
            module => fn i64_extend_8_s(value: others.ty) -> others.ty
        };

        let low = naga_expr!(module, function_handle => value[const 1] as Sint);
        let high = naga_expr!(module, function_handle => (low << U32(31)) >> U32(31));
        let low = naga_expr!(module, function_handle => (low << U32(24)) >> U32(24));
        let res = naga_expr!(module, function_handle => (others.ty)(high as Uint, low as Uint));
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }

    fn gen_extend_16_s(
        module: &mut naga::Module,
        others: i64_instance_gen::Extend16SRequirements,
    ) -> build::Result<i64_instance_gen::Extend16S> {
        let (function_handle, value) = declare_function! {
            module => fn i64_extend_16_s(value: others.ty) -> others.ty
        };

        let low = naga_expr!(module, function_handle => value[const 1] as Sint);
        let high = naga_expr!(module, function_handle => (low << U32(31)) >> U32(31));
        let low = naga_expr!(module, function_handle => (low << U32(16)) >> U32(16));
        let res = naga_expr!(module, function_handle => (others.ty)(high as Uint, low as Uint));
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }

    fn gen_extend_32_s(
        module: &mut naga::Module,
        others: i64_instance_gen::Extend32SRequirements,
    ) -> build::Result<i64_instance_gen::Extend32S> {
        let (function_handle, value) = declare_function! {
            module => fn i64_extend_16_s(value: others.ty) -> others.ty
        };

        let low = naga_expr!(module, function_handle => value[const 1]);
        let high = naga_expr!(module, function_handle => ((low as Sint) << U32(31)) >> U32(31));
        let res = naga_expr!(module, function_handle => (others.ty)(high as Uint, low));
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
