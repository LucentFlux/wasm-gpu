use crate::{
    build,
    std_objects::{preamble_objects_gen, wasm_tys::impl_native_bool_binexp},
};
use naga_ext::{
    declare_function, naga_expr, BlockContext, ConstantsExt, ExpressionsExt, TypesExt,
};

use super::{f32_instance_gen, F32Gen};


/// Takes a float and produces one that has been multiplied by 2^x, respecting denormals
/// using bitwise operations
fn scale_up_float(
    ctx: &mut BlockContext<'_>,
    value: naga::Handle<naga::Expression>,
    scale_amount: u8,
) -> naga::Handle<naga::Expression> {
    naga_expr!(ctx =>
        let value_u32 = bitcast<u32>(value);
        let exp = (value_u32 >> U32(23)) & U32(0xFF);
        if (exp == U32(0)) {
            // Manually build subnormal * 2^x
            let significand = value_u32 & U32(0x007FFFFF);
            if (significand == U32(0)) {
                // If significand is 0, value is 0
                value
            } else {
                let sign = value_u32 & U32(0x80000000);
                // Shift significand into normalised position, with first set bit to be discarded
                let to_shift = countLeadingZeros(significand) - U32(8);
                let norm_exp = (exp + U32(scale_amount as u32 + 1)) - to_shift;
                let norm_exp = norm_exp << U32(23);
                let norm_sgnf = (significand << to_shift) & U32(0x007FFFFF);
                bitcast<f32>(sign | norm_exp | norm_sgnf)
            }
        } else {
            // Multiply by 2^x if the value isn't a subnormal
            value * F32(f32::powi(2.0, scale_amount as i32))
        }
    )
}

/// Takes a float and produces one that has been multiplied by 2^-64, respecting denormals
/// using bitwise operations
fn scale_down_float(
    ctx: &mut BlockContext<'_>,
    value: naga::Handle<naga::Expression>,
    scale_amount: u8,
) -> naga::Handle<naga::Expression> {
    naga_expr!(ctx =>
        let value_u32 = bitcast<u32>(value);
        let sign = value_u32 & U32(0x80000000);
        let exp = (value_u32 >> U32(23)) & U32(0xFF);
        let significand = value_u32 & U32(0x007FFFFF);
        if (exp <= U32(scale_amount as u32)) {
            // Manually build subnormal * 2^-x
            //if ((exp == U32(0)) & (significand == U32(0))) {
                // If significand and exp are 0, value is 0
            //    value
            //} else {
                // Shift significand into denormalised position, with first set bit to be brought in
                let to_shift = U32(scale_amount as u32 + 1) - exp;
                if (to_shift >= U32(32)) {
                    bitcast<f32>(sign)
                } else {
                    let norm_sgnf = (significand | U32(0x00800000)) >> to_shift;
                    bitcast<f32>(sign | norm_sgnf)
                }
            //}
        } else {
            // Not needed because we multiply by 1/2^n rather than dividing
            /*if (Bool((exp >= U32(253)) & (exp != U32(255))) {
                // Manually handle numbers between 2^126 and inf
                let exp = exp - U32(scale_amount as u32);
                bitcast<f32>(sign | (exp << U32(23)) | significand)
            } else {*/
            // Multiply by 2^-x to bring result into non-subnormal range
            value * F32(f32::powi(2.0, -(scale_amount as i32)))

        }
    )
}

fn subnormal_add(
    ctx: &mut BlockContext<'_>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!(ctx => lhs + rhs);

    let res_var = ctx.new_local("subnormal_res", ty, None);
    let res_ptr = ctx.local_expr(res_var);
    ctx.store(res_ptr, res);

    let is_subnormal = naga_expr!(ctx => 
        (res >= F32(-9.861e-32)) & (res <= F32(9.861e-32))
    );
    ctx.test(is_subnormal).then(|mut ctx| {
        // Short-circuit if we're adding 0 to 0 (ignoring sign bit)
        let is_adding_zeros = naga_expr!{&mut ctx => 
            (bitcast<u32>(lhs) | bitcast<u32>(rhs) | U32(0x80000000)) == U32(0x80000000)
        };

        ctx.test(is_adding_zeros).then(|mut ctx| {
            let new_res = naga_expr!{&mut ctx => 
                bitcast<f32>(bitcast<u32>(lhs) & bitcast<u32>(rhs))
            };

            ctx.store(res_ptr, new_res);
        }).otherwise(|mut ctx| {
            // Short-circuit if we're adding n to -n
            let is_adding_n_to_minus_n = naga_expr!{&mut ctx => 
                (bitcast<u32>(lhs) ^ bitcast<u32>(rhs)) == U32(0x80000000)
            };

            ctx.test(is_adding_n_to_minus_n).then(|mut ctx| {
                let const_zero = ctx.literal_expr_from(0.0f32);
                ctx.store(res_ptr, const_zero);
            }).otherwise(|mut ctx| {
                // Scale both floats up
                let lhs_scaled = scale_up_float(&mut ctx, lhs, 64);
                let rhs_scaled = scale_up_float(&mut ctx, rhs, 64);

                let scaled_new_res = naga_expr!{&mut ctx => 
                    lhs_scaled + rhs_scaled
                };

                // Scale back down, possibly into subnormal range
                let new_res = scale_down_float(&mut ctx, scaled_new_res, 64);
                ctx.store(res_ptr, new_res);
            });
        });
    });

    return naga_expr!(ctx => Load(res_ptr));
}

fn subnormal_sub(
    ctx: &mut BlockContext<'_>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!{ctx => 
        lhs - rhs
    };

    let res_var = ctx.new_local("subnormal_res", ty, None);
    let res_ptr = ctx.local_expr(res_var);
    ctx.store(res_ptr, res);

    let is_subnormal = naga_expr!{ctx => 
        (res >= F32(-9.861e-32)) & (res <= F32(9.861e-32))
    };
    ctx.test(is_subnormal).then(|mut ctx| {
        // Short-circuit if we're subbing 0 from 0 (ignoring sign bit)
        let is_subbing_zeros = naga_expr!{&mut ctx => 
            (bitcast<u32>(lhs) | bitcast<u32>(rhs) | U32(0x80000000)) == U32(0x80000000)
        };
        ctx.test(is_subbing_zeros).then(|mut ctx| {
            let new_res = naga_expr!{&mut ctx => 
                bitcast<f32>(bitcast<u32>(lhs) & (bitcast<u32>(rhs) ^ U32(0x80000000)))
            };
            ctx.store(res_ptr, new_res);
        }).otherwise(|mut ctx| {
            // Short-circuit if we're subbing n from n
            let is_subbing_n_from_n = naga_expr!{&mut ctx => 
                bitcast<u32>(lhs) == bitcast<u32>(rhs)
            };

            ctx.test(is_subbing_n_from_n).then(|mut ctx| {
                let zero = naga_expr!(&mut ctx => F32(0.0));
                ctx.store(res_ptr, zero);
            }).otherwise(|mut ctx| {
                // Scale both floats up
                let lhs_scaled = scale_up_float(&mut ctx, lhs, 64);
                let rhs_scaled = scale_up_float(&mut ctx, rhs, 64);

                let scaled_new_res = naga_expr!{&mut ctx => 
                    lhs_scaled - rhs_scaled
                };

                // Scale back down, possibly into subnormal range
                let new_res = scale_down_float(&mut ctx, scaled_new_res, 64);
                ctx.store(res_ptr, new_res);
            });
        });
    });

    return naga_expr!(ctx => Load(res_ptr));
}

fn subnormal_mult(
    ctx: &mut BlockContext<'_>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    let lhs_u32 = naga_expr!{ctx => 
        bitcast<u32>(lhs)
    };
    let rhs_u32 = naga_expr!{ctx => 
        bitcast<u32>(rhs)
    };

    let lhs_exp = naga_expr!{ctx => 
        (lhs_u32 & U32(0x7F800000)) >> U32(23)
    };
    let rhs_exp = naga_expr!{ctx => 
        (rhs_u32 & U32(0x7F800000)) >> U32(23)
    };

    let is_nan_or_inf = naga_expr!{ctx => 
        (lhs_exp == U32(0xFF)) | (rhs_exp == U32(0xFF))
    };
    let is_zero = naga_expr!{ctx => 
        ((lhs_u32 & U32(0x7FFFFFFF)) == U32(0)) | ((rhs_u32 & U32(0x7FFFFFFF)) == U32(0))
    };

    let res_var = ctx.new_local("res", ty, None);
    let res_ptr = ctx.local_expr(res_var);

    let is_short_circuit = naga_expr!{ctx => 
        is_nan_or_inf | is_zero
    };
    ctx.test(is_short_circuit).then(|mut ctx| {
        let res = naga_expr!{&mut ctx => 
            if (is_nan_or_inf) {
                if (is_zero) {
                    lhs * rhs
                } else {
                    // Subnormal * value != 0.0 * value, so we need to make sub-normals something other than 0.0
                    bitcast<f32>(lhs_u32 | U32(0x10000000)) * bitcast<f32>(rhs_u32 | U32(0x10000000))
                }
            } else {
                bitcast<f32>((lhs_u32 ^ rhs_u32) & U32(0x80000000))
            }
        };
        ctx.store(res_ptr, res);
    }).otherwise(|mut ctx| {
        let can_just_mult = naga_expr!(&mut ctx =>
            (lhs_exp != U32(0)) & (rhs_exp != U32(0)) & ((lhs_exp + rhs_exp) > U32(127))
        );
        ctx.test(can_just_mult).then(|mut ctx| {
            let res = naga_expr!(&mut ctx => lhs * rhs);
            ctx.store(res_ptr, res);
        }).otherwise(|mut ctx| {
            let is_lhs_smaller = naga_expr!(&mut ctx =>
                lhs_exp <= rhs_exp
            );
            ctx.test(is_lhs_smaller).then(|mut ctx| {
                // Scale lhs up since it won't go to inf
                let lhs_scaled = scale_up_float(&mut ctx, lhs, 32);

                let scaled_new_res = naga_expr!(&mut ctx => lhs_scaled * rhs);

                // Scale back down, possibly into subnormal range
                let new_res = scale_down_float(&mut ctx, scaled_new_res, 32);

                ctx.store(res_ptr, new_res);
            }).otherwise(|mut ctx| {
                // Scale rhs up since it won't go to inf
                let rhs_scaled = scale_up_float(&mut ctx, rhs, 32);
                let scaled_new_res = naga_expr!(&mut ctx => lhs * rhs_scaled);

                // Scale back down, possibly into subnormal range
                let new_res = scale_down_float(&mut ctx, scaled_new_res, 32);

                ctx.store(res_ptr, new_res);
            });
        });
    });

    return naga_expr!(ctx => Load(res_ptr));
}

fn subnormal_div(
    ctx: &mut BlockContext<'_>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    let lhs_u32 = naga_expr!(ctx => bitcast<u32>(lhs));
    let rhs_u32 = naga_expr!(ctx => bitcast<u32>(rhs));

    let lhs_exp = naga_expr!(ctx => (lhs_u32 & U32(0x7F800000)) >> U32(23));
    let rhs_exp = naga_expr!(ctx => (rhs_u32 & U32(0x7F800000)) >> U32(23));

    let res_var = ctx.new_local("subnormal_res", ty, None);
    let res_ptr = ctx.local_expr(res_var);

    // The exponent of the result, without the -127 factor
    let res_exp_no_offset = naga_expr!{ctx => 
        bitcast<i32>(lhs_exp) - bitcast<i32>(rhs_exp)
    };
    let is_lhs_subnormal_or_zero = naga_expr!(ctx => lhs_exp == U32(0));
    let is_rhs_subnormal_or_zero = naga_expr!(ctx => rhs_exp == U32(0));
    let is_either_operand_subnormal_or_zero = naga_expr!{ctx => 
        is_lhs_subnormal_or_zero | is_rhs_subnormal_or_zero
    };

    // Cutoff is -126 + 24 since subnormals have a read exponent of -126 but may have a smaller actual exponent.
    // Two subnormals don't need to be accounted for, since they will definitely be below this limit
    let can_divide_normally = naga_expr!{ctx => 
        (res_exp_no_offset > I32(-102)) & !is_either_operand_subnormal_or_zero
    };
    ctx.test(can_divide_normally).then(|mut ctx| {
        let res = naga_expr!(&mut ctx => lhs / rhs);
        ctx.store(res_ptr, res);
    }).otherwise(|mut ctx| {
        // Either the lhs or rhs are subnormal, or the result exponent is small.
        // For 1 & 3, we want to shift up the lhs. For 2, if the lhs goes to inf when
        // shifted then it was going to inf anyway, so shifting is safe.
        let lhs_scaled = scale_up_float(&mut ctx, lhs, 32);
        ctx.test(is_rhs_subnormal_or_zero).then(|mut ctx| {
            // If the rhs is subnormal, when we scale up then we don't need to scale down the result
            let rhs_scaled = scale_up_float(&mut ctx, rhs, 32);
            let res = naga_expr!(&mut ctx => lhs_scaled / rhs_scaled);
            ctx.store(res_ptr, res);
        }).otherwise(|mut ctx| {
            let is_rhs_tiny = naga_expr!(&mut ctx => rhs_exp <= U32(64));
            ctx.test(is_rhs_tiny).then(|mut ctx| {
                let res_scaled = naga_expr!(&mut ctx => lhs_scaled / rhs);
                let res = scale_down_float(&mut ctx, res_scaled, 32);
                ctx.store(res_ptr, res);
            }).otherwise(|mut ctx| {
                let rhs_scaled = scale_down_float(&mut ctx, rhs, 32);
                let res_scaled = naga_expr!(&mut ctx => lhs_scaled / rhs_scaled);
                let res = scale_down_float(&mut ctx, res_scaled, 64);
                ctx.store(res_ptr, res);
            });
        });
    });

    return naga_expr!(ctx => Load(res_ptr));
}

fn subnormal_sqrt(
    ctx: &mut BlockContext<'_>,
    ty: naga::Handle<naga::Type>,
    value: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!(ctx => sqrt(value));

    let res_var = ctx.new_local("subnormal_res", ty, None);
    let res_ptr = ctx.local_expr(res_var);
    ctx.store(res_ptr, res);

    let is_subnormal = naga_expr!{ctx => 
        (res == F32(0.0)) & ((bitcast<u32>(value) | U32(0x80000000)) != U32(0x80000000))
    };
    ctx.test(is_subnormal).then(|mut ctx| {
        let value_scaled = scale_up_float(&mut ctx, value, 64);

        let res_scaled = naga_expr!(&mut ctx => sqrt(value_scaled));

        let res = scale_down_float(&mut ctx, res_scaled, 32);

        ctx.store(res_ptr, res);
    });

    return naga_expr!(ctx => Load(res_ptr));
}

fn subnormal_min(
    ctx: &mut BlockContext<'_>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!(ctx => min(lhs, rhs));

    let res_var =ctx.new_local("subnormal_res", ty, None);
    let res_ptr = ctx.local_expr(res_var);
    ctx.store(res_ptr, res);

    let is_subnormal = naga_expr!{ctx => 
        (res >= F32(-9.861e-32)) & (res <= F32(9.861e-32))
    };
    ctx.test(is_subnormal).then(|mut ctx| {
        // Implement min manually
        let new_res = naga_expr!{&mut ctx =>
            // If we flip the bits in a negative float, and the sign bits, the min is just the min of the integers of the representations
            // (ignoring NaNs, as they are fixed in the min body impl since WebGPU also ignores NaNs)

            let lhs_u32 = bitcast<u32>(lhs);
            let rhs_u32 = bitcast<u32>(rhs);

            let lhs_xor_mask = (lhs_u32 >> U32(31)) * U32(0x7FFFFFFF);
            let lhs_u32 = lhs_u32 ^ lhs_xor_mask;
            let lhs_u32 = lhs_u32 ^ U32(0x80000000);
            let rhs_xor_mask = (rhs_u32 >> U32(31)) * U32(0x7FFFFFFF);
            let rhs_u32 = rhs_u32 ^ rhs_xor_mask;
            let rhs_u32 = rhs_u32 ^ U32(0x80000000);

            let min = min(lhs_u32, rhs_u32);

            let min = min ^ U32(0x80000000);
            let min_xor_mask = (min >> U32(31)) * U32(0x7FFFFFFF);
            let min = min ^ min_xor_mask;

            // Account for inf
            if (lhs_u32 == U32(0x7F800000)) {
                rhs
            } else {
                if (rhs_u32 == U32(0x7F800000)) {
                    lhs
                } else {
                    bitcast<f32>(min)
                }
            }
        };

        ctx.store(res_ptr, new_res);
    });

    return naga_expr!(ctx => Load(res_ptr));
}

fn subnormal_max(
    ctx: &mut BlockContext<'_>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!(ctx => max(lhs, rhs));

    let res_var = ctx.new_local("subnormal_res", ty, None);
    let res_ptr = ctx.local_expr(res_var);
    ctx.store(res_ptr, res);

    let is_subnormal = naga_expr!{ctx => 
        (res >= F32(-9.861e-32)) & (res <= F32(9.861e-32))
    };
    ctx.test(is_subnormal).then(|mut ctx| {
        // Implement min manually
        let new_res = naga_expr!{&mut ctx =>
            // If we flip the sign bit, the min is just the min of the integers of the representations
            // (ignoring NaNs, as they are fixed in the min body impl since WebGPU also ignores NaNs)

            let lhs_u32 = bitcast<u32>(lhs);
            let rhs_u32 = bitcast<u32>(rhs);

            let lhs_xor_mask = (lhs_u32 >> U32(31)) * U32(0x7FFFFFFF);
            let lhs_u32 = lhs_u32 ^ lhs_xor_mask;
            let lhs_u32 = lhs_u32 ^ U32(0x80000000);
            let rhs_xor_mask = (rhs_u32 >> U32(31)) * U32(0x7FFFFFFF);
            let rhs_u32 = rhs_u32 ^ rhs_xor_mask;
            let rhs_u32 = rhs_u32 ^ U32(0x80000000);

            let max = max(lhs_u32, rhs_u32);

            let max = max ^ U32(0x80000000);
            let max_xor_mask = (max >> U32(31)) * U32(0x7FFFFFFF);
            let max = max ^ max_xor_mask;

            // Account for -inf
            if (lhs_u32 == U32(0xFF800000)) {
                rhs
            } else {
                if (rhs_u32 == U32(0xFF800000)) {
                    lhs
                } else {
                    bitcast<f32>(max)
                }
            }
        };

        ctx.store(res_ptr, new_res);
    });

    return naga_expr!(ctx => Load(res_ptr));
}

/// An implementation of f32s using the GPU's native f32 type
pub(crate) struct NativeF32;
impl F32Gen for NativeF32 {
    fn gen_ty(
        module: &mut naga::Module,
        _requirements: super::f32_instance_gen::TyRequirements,
    ) -> build::Result<super::f32_instance_gen::Ty> {
        Ok(module.types.insert_f32())
    }

    fn gen_default(
        module: &mut naga::Module,
        requirements: super::f32_instance_gen::DefaultRequirements,
    ) -> build::Result<super::f32_instance_gen::Default> {
        let expr = module.const_expressions.append_f32(0.0);
        let res = module.constants.append_anonymous(*requirements.ty, expr);
        Ok(res)
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _requirements: super::f32_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::f32_instance_gen::SizeBytes> {
        Ok(4)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        _requirements: super::f32_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::f32_instance_gen::MakeConst> {
        Ok(Box::new(|const_expressions, value| {
            Ok(const_expressions.append_f32(value))
        }))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        requirements: super::f32_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::f32_instance_gen::ReadInput> {
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
        requirements: super::f32_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::f32_instance_gen::WriteOutput> {
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
        requirements: super::f32_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::f32_instance_gen::ReadMemory> {
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
        requirements: super::f32_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::f32_instance_gen::WriteMemory> {
        gen_write(
            module,
            requirements.preamble.word_ty,
            *requirements.ty,
            requirements.preamble.bindings.memory,
            "memory",
        )
    }

    fn gen_abs(
        module: &mut naga::Module,
        requirements: f32_instance_gen::AbsRequirements,
    ) -> build::Result<f32_instance_gen::Abs> {
        let (function_handle, value) = declare_function! {
            module => fn f32_abs(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // Just unset sign bit
        let res = naga_expr!(&mut ctx => bitcast<f32>(bitcast<u32>(value) & U32(0x7FFFFFFF)));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_neg(
        module: &mut naga::Module,
        requirements: f32_instance_gen::NegRequirements,
    ) -> build::Result<f32_instance_gen::Neg> {
        let (function_handle, value) = declare_function! {
            module => fn f32_neg(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // Just flip sign bit
        let res = naga_expr!(&mut ctx => bitcast<f32>(bitcast<u32>(value) ^ U32(0x80000000)));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_ceil(
        module: &mut naga::Module,
        requirements: f32_instance_gen::CeilRequirements,
    ) -> build::Result<f32_instance_gen::Ceil> {
        let (function_handle, value) = declare_function! {
            module => fn f32_ceil(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = if requirements.fp_options.emulate_subnormals {
            naga_expr!(&mut ctx =>
                let value_u32 = bitcast<u32>(value);
                let exp = (value_u32 >> U32(23)) & U32(0xFF);
                if (exp == U32(0)) {
                    let sign = value_u32 & U32(0x80000000);
                    let significand = value_u32 & U32(0x007FFFFF);
                    if ((sign == U32(0)) & (significand != U32(0))) {
                        F32(1.0)
                    } else {
                        bitcast<f32>(U32((0.0f32).to_bits()) | sign)
                    }
                } else {
                    ceil(value)
                }
            )
        } else {
            naga_expr!(&mut ctx => ceil(value))
        };
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_floor(
        module: &mut naga::Module,
        requirements: f32_instance_gen::FloorRequirements,
    ) -> build::Result<f32_instance_gen::Floor> {
        let (function_handle, value) = declare_function! {
            module => fn f32_floor(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = if requirements.fp_options.emulate_subnormals {
            naga_expr!(&mut ctx =>
                let value_u32 = bitcast<u32>(value);
                let exp = (value_u32 >> U32(23)) & U32(0xFF);
                if (exp == U32(0)) {
                    let sign = value_u32 & U32(0x80000000);
                    let significand = value_u32 & U32(0x007FFFFF);
                    if ((sign != U32(0)) & (significand != U32(0))) {
                        F32(-1.0)
                    } else {
                        floor(value)
                    }
                } else {
                    floor(value)
                }
            )
        } else {
            naga_expr!(&mut ctx => floor(value))
        };
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_trunc(
        module: &mut naga::Module,
        requirements: f32_instance_gen::TruncRequirements,
    ) -> build::Result<f32_instance_gen::Trunc> {
        let (function_handle, value) = declare_function! {
            module => fn f32_trunc(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // No subnormal correction required because `trunc(±ε) == ±0.0`
        let res = naga_expr!(&mut ctx => trunc(value));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_nearest(
        module: &mut naga::Module,
        requirements: f32_instance_gen::NearestRequirements,
    ) -> build::Result<f32_instance_gen::Nearest> {
        let (function_handle, value) = declare_function! {
            module => fn f32_nearest(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // No subnormal correction required because `nearest(±ε) == ±0.0`
        let res = naga_expr!(&mut ctx => round(value));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_sqrt(
        module: &mut naga::Module,
        requirements: f32_instance_gen::SqrtRequirements,
    ) -> build::Result<f32_instance_gen::Sqrt> {
        let (function_handle, value) = declare_function! {
            module => fn f32_sqrt(value: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = if requirements.fp_options.emulate_subnormals {
            let subnormal_sqrt = subnormal_sqrt(&mut ctx, *requirements.ty, value);
            naga_expr!(&mut ctx =>
                let value_u32 = bitcast<u32>(value);
                let sign = value_u32 & U32(0x80000000);
                if (sign != U32(0)) {
                    if (value_u32 == U32(0x80000000)) {
                        value // sqrt -0.0 is -0.0
                    } else {
                        F32(f32::NAN)
                    }
                } else {
                    subnormal_sqrt
                }
            )
        } else {
            naga_expr!(&mut ctx => sqrt(value))
        };
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_min(
        module: &mut naga::Module,
        requirements: f32_instance_gen::MinRequirements,
    ) -> build::Result<f32_instance_gen::Min> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn f32_min(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let min = if requirements.fp_options.emulate_subnormals {
            subnormal_min(&mut ctx, *requirements.ty, lhs, rhs)
        } else {
            naga_expr!(&mut ctx => min(lhs, rhs))
        };

        // Inbuilt `min` doesn't respect NaNs
        let is_nan = naga_expr!(&mut ctx => 
            (((bitcast<u32>(lhs) & U32(0x7FFFFFFF)) > U32(0x7f800000)) | ((bitcast<u32>(rhs) & U32(0x7FFFFFFF)) > U32(0x7f800000)))
        );
        let res = naga_expr!(&mut ctx => if (is_nan) {bitcast<f32>(U32(0x7fc00000))} else { min });
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_max(
        module: &mut naga::Module,
        requirements: f32_instance_gen::MaxRequirements,
    ) -> build::Result<f32_instance_gen::Max> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn f32_max(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let max = if requirements.fp_options.emulate_subnormals {
            subnormal_max(&mut ctx, *requirements.ty, lhs, rhs)
        } else {
            naga_expr!(&mut ctx => max(lhs, rhs))
        };

        // Inbuilt `max` doesn't respect NaNs
        let is_nan = naga_expr!(&mut ctx => (((bitcast<u32>(lhs) & U32(0x7FFFFFFF)) > U32(0x7f800000)) | ((bitcast<u32>(rhs) & U32(0x7FFFFFFF)) > U32(0x7f800000))));
        let res = naga_expr!(&mut ctx => if (is_nan) {bitcast<f32>(U32(0x7fC00000))} else { max });
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_copy_sign(
        module: &mut naga::Module,
        requirements: f32_instance_gen::CopySignRequirements,
    ) -> build::Result<f32_instance_gen::CopySign> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn f32_copy_sign(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = naga_expr!(&mut ctx => sign(lhs) * sign(rhs) * lhs);
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_add(
        module: &mut naga::Module,
        requirements: f32_instance_gen::AddRequirements,
    ) -> build::Result<f32_instance_gen::Add> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn f32_add(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = if requirements.fp_options.emulate_subnormals {
            subnormal_add(&mut ctx, *requirements.ty, lhs, rhs)
        } else {
            naga_expr!(&mut ctx => lhs + rhs)
        };
        ctx.result(res);

        Ok(function_handle)
    }
    fn gen_sub(
        module: &mut naga::Module,
        requirements: f32_instance_gen::SubRequirements,
    ) -> build::Result<f32_instance_gen::Sub> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn f32_sub(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = if requirements.fp_options.emulate_subnormals {
            subnormal_sub(&mut ctx, *requirements.ty, lhs, rhs)
        } else {
            naga_expr!(&mut ctx => lhs - rhs)
        };
        ctx.result(res);

        Ok(function_handle)
    }
    fn gen_mul(
        module: &mut naga::Module,
        requirements: f32_instance_gen::MulRequirements,
    ) -> build::Result<f32_instance_gen::Mul> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn f32_mul(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        let res = if requirements.fp_options.emulate_subnormals {
            subnormal_mult(&mut ctx, *requirements.ty, lhs, rhs)
        } else {
            naga_expr!(&mut ctx => lhs * rhs)
        };
        ctx.result(res);

        Ok(function_handle)
    }

    impl_native_bool_binexp! { f32_instance_gen, f32, eq; == }
    impl_native_bool_binexp! { f32_instance_gen, f32, ne; != }

    fn gen_div(
        module: &mut naga::Module,
        requirements: f32_instance_gen::DivRequirements,
    ) -> build::Result<f32_instance_gen::Div> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn f32_div(lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        if requirements.fp_options.emulate_div_beyond_max {
            let lhs_exp = naga_expr!(&mut ctx => (bitcast<u32>(lhs) >> U32(23)) & U32(0xFF));
            let rhs_exp = naga_expr!(&mut ctx => (bitcast<u32>(rhs) >> U32(23)) & U32(0xFF));
            let is_lhs_beyond_max = naga_expr!(&mut ctx => (lhs_exp >= U32(253)) & (lhs_exp != U32(255)));
            let is_rhs_beyond_max = naga_expr!(&mut ctx => (rhs_exp >= U32(253)) & (rhs_exp != U32(255)));
            let is_both_beyond_max = naga_expr!(&mut ctx => is_lhs_beyond_max & is_rhs_beyond_max);

            ctx.test(is_both_beyond_max).then(|mut ctx| {
                let lhs_scaled = scale_down_float(&mut ctx, lhs, 32);
                let rhs_scaled = scale_down_float(&mut ctx, rhs, 32);
                let res = naga_expr!(&mut ctx => lhs_scaled / rhs_scaled);
                ctx.result(res);
            });

            ctx.test(is_lhs_beyond_max).then(|mut ctx| {
                let lhs_scaled = scale_down_float(&mut ctx, lhs, 32);
                let res_scaled = naga_expr!(&mut ctx => lhs_scaled / rhs);
                let res = scale_up_float(&mut ctx, res_scaled, 32);
                ctx.result(res);
            });

            ctx.test(is_rhs_beyond_max).then(|mut ctx| {
                let rhs_scaled = scale_down_float(&mut ctx, rhs, 32);
                let res_scaled = naga_expr!(&mut ctx => lhs / rhs_scaled);
                let res = scale_up_float(&mut ctx, res_scaled, 32);
                ctx.result(res);
            });
        }

        let res = if requirements.fp_options.emulate_subnormals {
            subnormal_div(&mut ctx, *requirements.ty, lhs, rhs)
        } else {
            naga_expr!(&mut ctx => lhs / rhs)
        };
        ctx.result(res);

        Ok(function_handle)
    }

    super::impl_load_and_store! {f32_instance_gen, f32}

    fn gen_convert_i32_s(
        module: &mut naga::Module,
        requirements: f32_instance_gen::ConvertI32SRequirements,
    ) -> build::Result<f32_instance_gen::ConvertI32S> {
        let (function_handle, value) = declare_function! {
            module => fn f32_convert_i32_s(value: *requirements.i32_ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // No subnormal correction required because integers can't be subnormals
        let res = naga_expr!(&mut ctx => f32(value));
        ctx.result(res);

        Ok(function_handle)
    }

    fn gen_convert_i32_u(
        module: &mut naga::Module,
        requirements: f32_instance_gen::ConvertI32URequirements,
    ) -> build::Result<f32_instance_gen::ConvertI32U> {
        let (function_handle, value) = declare_function! {
            module => fn f32_convert_i32_s(value: *requirements.i32_ty) -> *requirements.ty
        };
        let mut ctx = BlockContext::from((module, function_handle));

        // No subnormal correction required because integers can't be subnormals
        let res = naga_expr!(&mut ctx => f32(bitcast<u32>(value)));
        ctx.result(res);

        Ok(function_handle)
    }

    impl_native_bool_binexp! { f32_instance_gen, f32, lt; < }
    impl_native_bool_binexp! { f32_instance_gen, f32, le; <= }
    impl_native_bool_binexp! { f32_instance_gen, f32, gt; > }
    impl_native_bool_binexp! { f32_instance_gen, f32, ge; >= }
}

// fn<buffer>(word_address: u32) -> f32
fn gen_read(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    f32_ty: f32_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_f32_from_{}", buffer_name);

    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> f32_ty
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let read_word = naga_expr!(&mut ctx => Global(buffer)[word_address]);
    let read_value = naga_expr!(&mut ctx => Load(read_word) as Float);
    ctx.result(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: f32)
fn gen_write(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    f32_ty: f32_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_f32_to_{}", buffer_name);
    let (function_handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: f32_ty)
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let write_word_loc = naga_expr!(&mut ctx => Global(buffer)[word_address]);
    let word = naga_expr!(&mut ctx => value as Uint);
    ctx.store(write_word_loc, word);

    Ok(function_handle)
}
