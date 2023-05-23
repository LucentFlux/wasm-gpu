use std::sync::Arc;

use crate::{
    build,
    std_objects::{std_objects_gen, wasm_tys::impl_native_bool_binexp},
    FloatingPointOptions,
};
use naga_ext::{
    declare_function, naga_expr, BlockExt, ExpressionsExt, LocalsExt, ModuleExt, ShaderPart,
};

use super::{f32_instance_gen, F32Gen};

fn make_const_impl(
    constants: &mut naga::Arena<naga::Constant>,
    value: f32,
) -> naga::Handle<naga::Constant> {
    constants.append(
        naga::Constant {
            name: None,
            specialization: None,
            inner: naga::ConstantInner::Scalar {
                width: 4,
                value: naga::ScalarValue::Float(value.into()),
            },
        },
        naga::Span::UNDEFINED,
    )
}

/// Takes a float and produces one that has been multiplied by 2^x, respecting denormals
/// using bitwise operations
fn scale_up_float(
    part: &mut impl ShaderPart,
    value: naga::Handle<naga::Expression>,
    scale_amount: u8,
) -> naga::Handle<naga::Expression> {
    naga_expr!(part =>
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
    part: &mut impl ShaderPart,
    value: naga::Handle<naga::Expression>,
    scale_amount: u8,
) -> naga::Handle<naga::Expression> {
    naga_expr!(part =>
        let value_u32 = bitcast<u32>(value);
        let sign = value_u32 & U32(0x80000000);
        let exp = (value_u32 >> U32(23)) & U32(0xFF);
        let significand = value_u32 & U32(0x007FFFFF);
        if (exp <= U32(scale_amount as u32)) {
            // Manually build subnormal * 2^-x
            if ((exp == U32(0)) & (significand == U32(0))) {
                // If significand and exp are 0, value is 0
                value
            } else {
                // Shift significand into denormalised position, with first set bit to be brought in
                let to_shift = U32(scale_amount as u32 + 1) - exp;
                if (to_shift >= U32(32)) {
                    bitcast<f32>(sign)
                } else {
                    let norm_sgnf = (significand | U32(0x00800000)) >> to_shift;
                    bitcast<f32>(sign | norm_sgnf)
                }
            }
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
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!(module, function_handle => lhs + rhs);

    let res_var =
        module
            .fn_mut(function_handle)
            .local_variables
            .new_local("subnormal_res", ty, None);
    let res_ptr = module
        .fn_mut(function_handle)
        .expressions
        .append_local(res_var);
    module.fn_mut(function_handle).body.push_store(res_ptr, res);

    let is_subnormal = naga_expr!(module, function_handle => (res >= F32(-9.861e-32))
                                         & (res <= F32(9.861e-32))
    );

    let mut if_subnormal = naga::Block::default();
    {
        // Short-circuit if we're adding 0 to 0 (ignoring sign bit)
        let is_adding_zeros = naga_expr!(module, function_handle => (bitcast<u32>(lhs) | bitcast<u32>(rhs) | U32(0x80000000)) == U32(0x80000000));

        let mut if_adding_zeros = naga::Block::default();
        {
            let new_res = naga_expr!(module, function_handle, if_adding_zeros => bitcast<f32>(bitcast<u32>(lhs) & bitcast<u32>(rhs)));

            if_adding_zeros.push_store(res_ptr, new_res);
        }

        let mut if_not_adding_zeros = naga::Block::default();
        {
            // Short-circuit if we're adding n to -n
            let is_adding_n_to_minus_n = naga_expr!(module, function_handle, if_not_adding_zeros => (bitcast<u32>(lhs) ^ bitcast<u32>(rhs)) == U32(0x80000000));

            let mut if_adding_n_to_minus_n = naga::Block::default();
            {
                let zero = naga_expr!(module, function_handle, if_not_adding_zeros => F32(0.0));
                if_adding_n_to_minus_n.push_store(res_ptr, zero);
            }

            let mut if_not_adding_n_to_minus_n = naga::Block::default();
            {
                // Scale both floats up
                let lhs_scaled = scale_up_float(
                    &mut (
                        &mut *module,
                        function_handle,
                        &mut if_not_adding_n_to_minus_n,
                    ),
                    lhs,
                    64,
                );
                let rhs_scaled = scale_up_float(
                    &mut (
                        &mut *module,
                        function_handle,
                        &mut if_not_adding_n_to_minus_n,
                    ),
                    rhs,
                    64,
                );

                let scaled_new_res = naga_expr!(module, function_handle, if_not_adding_n_to_minus_n => lhs_scaled + rhs_scaled);

                // Scale back down, possibly into subnormal range
                let new_res = scale_down_float(
                    &mut (
                        &mut *module,
                        function_handle,
                        &mut if_not_adding_n_to_minus_n,
                    ),
                    scaled_new_res,
                    64,
                );

                if_not_adding_n_to_minus_n.push_store(res_ptr, new_res);
            }

            if_not_adding_zeros.push_if(
                is_adding_n_to_minus_n,
                if_adding_n_to_minus_n,
                if_not_adding_n_to_minus_n,
            );
        }

        if_subnormal.push_if(is_adding_zeros, if_adding_zeros, if_not_adding_zeros);
    }

    module
        .fn_mut(function_handle)
        .body
        .push_if(is_subnormal, if_subnormal, naga::Block::default());

    return naga_expr!(module, function_handle => Load(res_ptr));
}

fn subnormal_sub(
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!(module, function_handle => lhs - rhs);

    let res_var =
        module
            .fn_mut(function_handle)
            .local_variables
            .new_local("subnormal_res", ty, None);
    let res_ptr = module
        .fn_mut(function_handle)
        .expressions
        .append_local(res_var);
    module.fn_mut(function_handle).body.push_store(res_ptr, res);

    let is_subnormal = naga_expr!(module, function_handle => (res >= F32(-9.861e-32))
                                         & (res <= F32(9.861e-32))
    );

    let mut if_subnormal = naga::Block::default();
    {
        // Short-circuit if we're subbing 0 from 0 (ignoring sign bit)
        let is_subbing_zeros = naga_expr!(module, function_handle => (bitcast<u32>(lhs) | bitcast<u32>(rhs) | U32(0x80000000)) == U32(0x80000000));

        let mut if_subbing_zeros = naga::Block::default();
        {
            let new_res = naga_expr!(module, function_handle, if_subbing_zeros => bitcast<f32>(bitcast<u32>(lhs) & (bitcast<u32>(rhs) ^ U32(0x80000000))));

            if_subbing_zeros.push_store(res_ptr, new_res);
        }

        let mut if_not_subbing_zeros = naga::Block::default();
        {
            // Short-circuit if we're subbing n from n
            let is_subbing_n_from_n = naga_expr!(module, function_handle, if_not_subbing_zeros => bitcast<u32>(lhs) == bitcast<u32>(rhs));

            let mut if_subbing_n_from_n = naga::Block::default();
            {
                let zero = naga_expr!(module, function_handle, if_subbing_n_from_n => F32(0.0));
                if_subbing_n_from_n.push_store(res_ptr, zero);
            }

            let mut if_not_subbing_n_from_n = naga::Block::default();
            {
                // Scale both floats up
                let lhs_scaled = scale_up_float(
                    &mut (&mut *module, function_handle, &mut if_not_subbing_n_from_n),
                    lhs,
                    64,
                );
                let rhs_scaled = scale_up_float(
                    &mut (&mut *module, function_handle, &mut if_not_subbing_n_from_n),
                    rhs,
                    64,
                );

                let scaled_new_res = naga_expr!(module, function_handle, if_not_subbing_n_from_n => lhs_scaled - rhs_scaled);

                // Scale back down, possibly into subnormal range
                let new_res = scale_down_float(
                    &mut (&mut *module, function_handle, &mut if_not_subbing_n_from_n),
                    scaled_new_res,
                    64,
                );

                if_not_subbing_n_from_n.push_store(res_ptr, new_res);
            }

            if_not_subbing_zeros.push_if(
                is_subbing_n_from_n,
                if_subbing_n_from_n,
                if_not_subbing_n_from_n,
            );
        }

        if_subnormal.push_if(is_subbing_zeros, if_subbing_zeros, if_not_subbing_zeros);
    }

    module
        .fn_mut(function_handle)
        .body
        .push_if(is_subnormal, if_subnormal, naga::Block::default());

    return naga_expr!(module, function_handle => Load(res_ptr));
}

fn subnormal_mult(
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    let lhs_u32 = naga_expr!(module, function_handle => bitcast<u32>(lhs));
    let rhs_u32 = naga_expr!(module, function_handle => bitcast<u32>(rhs));

    let lhs_exp = naga_expr!(module, function_handle => (lhs_u32 & U32(0x7F800000)) >> U32(23));
    let rhs_exp = naga_expr!(module, function_handle => (rhs_u32 & U32(0x7F800000)) >> U32(23));

    let is_nan_or_inf =
        naga_expr!(module, function_handle => (lhs_exp == U32(0xFF)) | (rhs_exp == U32(0xFF)));
    let is_zero = naga_expr!(module, function_handle => ((lhs_u32 & U32(0x7FFFFFFF)) == U32(0)) | ((rhs_u32 & U32(0x7FFFFFFF)) == U32(0)));

    let res_var = module
        .fn_mut(function_handle)
        .local_variables
        .new_local("res", ty, None);
    let res_ptr = module
        .fn_mut(function_handle)
        .expressions
        .append_local(res_var);

    let is_short_circuit = naga_expr!(module, function_handle => is_nan_or_inf | is_zero);

    let mut if_short_circuit = naga::Block::default();
    {
        let res = naga_expr!(module, function_handle, if_short_circuit => if (is_nan_or_inf) {
            // Subnormal * value != 0.0 * value, so we need to make sub-normals something other than 0.0
            let lhs_sgnf = lhs_u32 & U32(0x007FFFFF);
            let rhs_sgnf = rhs_u32 & U32(0x007FFFFF);
            let lhs = if (lhs_sgnf != U32(0)) {bitcast<f32>(lhs_u32 | U32(0x10000000))} else {lhs};
            let rhs = if (rhs_sgnf != U32(0)) {bitcast<f32>(rhs_u32 | U32(0x10000000))} else {rhs};
            lhs * rhs
        } else {
            bitcast<f32>((lhs_u32 ^ rhs_u32) & U32(0x80000000))
        });
        if_short_circuit.push_store(res_ptr, res);
    }

    let mut if_not_short_circuit = naga::Block::default();
    {
        // Manual multiplication impl
        let res = naga_expr!(module, function_handle, if_not_short_circuit =>
            let lhs_sgnf = lhs_u32 & U32(0x007FFFFF);
            let rhs_sgnf = rhs_u32 & U32(0x007FFFFF);

            // Used when normalising
            let lhs_shift = countLeadingZeros(lhs_sgnf) - U32(8);
            let rhs_shift = countLeadingZeros(rhs_sgnf) - U32(8);

            // Caluclate fp with exponent set to 127 (* 2^0)
            let lhs_shifted = if (lhs_exp == U32(0)) {
                let lhs_sgnf = (lhs_sgnf << lhs_shift) & U32(0x007FFFFF);
                let lhs_sign = lhs_u32 & U32(0x80000000);
                lhs_sign | lhs_sgnf
            } else {
                // Mask out exponent
                lhs_u32 & U32(0x807FFFFF)
            };
            let lhs_shifted = lhs_shifted | U32(0x3F800000);

            let rhs_shifted = if (rhs_exp == U32(0)) {
                let rhs_sgnf = (rhs_sgnf << rhs_shift) & U32(0x007FFFFF);
                let rhs_sign = rhs_u32 & U32(0x80000000);
                rhs_sign | rhs_sgnf
            } else {
                // Mask out exponent
                rhs_u32 & U32(0x807FFFFF)
            };
            let rhs_shifted = rhs_shifted | U32(0x3F800000);

            // Multiply, getting a result between -4.0 and 4.0
            let res_shifted = bitcast<f32>(lhs_shifted) * bitcast<f32>(rhs_shifted);
            let res_shifted = bitcast<u32>(res_shifted);

            let res_shifted_exp = (res_shifted & U32(0x7F800000)) >> U32(23);

            // Add back in exponents
            let lhs_exp_change = if (lhs_exp == U32(0)) {
                lhs_shift
            } else {
                U32(1)
            };
            let rhs_exp_change = if (rhs_exp == U32(0)) {
                rhs_shift
            } else {
                U32(1)
            };

            let exp_pve = lhs_exp + rhs_exp + res_shifted_exp;
            let exp_nve = lhs_exp_change + rhs_exp_change + U32(127 + 125);
            let res_u32 = if (exp_pve <= exp_nve) {
                let exp_rev = exp_nve - exp_pve;
                if (exp_rev >= U32(32)) {
                    // Result is zero
                    (lhs_u32 ^ rhs_u32) & U32(0x80000000)
                } else {
                    // Result is subnormal
                    let res_sign = res_shifted & U32(0x80000000);
                    let res_sgnf = (res_shifted & U32(0x007FFFFF)) | U32(0x00800000);
                    let res_sgnf = res_sgnf >> (exp_rev + U32(1));

                    res_sign | res_sgnf
                }
            } else {
                let res_exp = exp_pve - exp_nve;
                if (res_exp >= U32(255)) {
                    // Result is inf
                    let res_sign = res_shifted & U32(0x80000000);
                    res_sign | U32(0x7F800000)
                } else {
                    // Result is normal
                    let res_shifted = res_shifted & U32(0x807FFFFF);
                    res_shifted | (res_exp << U32(23))
                }
            };
            bitcast<f32>(res_u32)
        );
        if_not_short_circuit.push_store(res_ptr, res);
    }

    module.fn_mut(function_handle).body.push_if(
        is_short_circuit,
        if_short_circuit,
        if_not_short_circuit,
    );

    return naga_expr!(module, function_handle => Load(res_ptr));
}

fn subnormal_div(
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    let lhs_u32 = naga_expr!(module, function_handle => bitcast<u32>(lhs));
    let rhs_u32 = naga_expr!(module, function_handle => bitcast<u32>(rhs));

    let lhs_exp = naga_expr!(module, function_handle => (lhs_u32 & U32(0x7F800000)) >> U32(23));
    let rhs_exp = naga_expr!(module, function_handle => (rhs_u32 & U32(0x7F800000)) >> U32(23));

    let res_var =
        module
            .fn_mut(function_handle)
            .local_variables
            .new_local("subnormal_res", ty, None);
    let res_ptr = module
        .fn_mut(function_handle)
        .expressions
        .append_local(res_var);

    // The exponent of the result, without the -127 factor
    let res_exp_no_offset =
        naga_expr!(module, function_handle => bitcast<i32>(lhs_exp) - bitcast<i32>(rhs_exp));
    let is_lhs_subnormal_or_zero = naga_expr!(module, function_handle => lhs_exp == U32(0));
    let is_rhs_subnormal_or_zero = naga_expr!(module, function_handle => rhs_exp == U32(0));
    let is_either_operand_subnormal_or_zero =
        naga_expr!(module, function_handle => is_lhs_subnormal_or_zero | is_rhs_subnormal_or_zero);

    // Cutoff is -126 + 24 since subnormals have a read exponent of -126 but may have a smaller actual exponent.
    // Two subnormals don't need to be accounted for, since they will definitely be below this limit
    let can_divide_normally = naga_expr!(module, function_handle => (res_exp_no_offset > I32(-102)) & !is_either_operand_subnormal_or_zero);

    let mut if_divide_normally = naga::Block::default();
    {
        let res = naga_expr!(module, function_handle, if_divide_normally => lhs / rhs);

        if_divide_normally.push_store(res_ptr, res);
    }

    let mut if_not_divide_normally = naga::Block::default();
    {
        // Either the lhs or rhs are subnormal, or the result exponent is small.
        // For 1 & 3, we want to shift up the lhs. For 2, if the lhs goes to inf when
        // shifted then it was going to inf anyway, so shifting is safe.
        let lhs_scaled = scale_up_float(
            &mut (&mut *module, function_handle, &mut if_not_divide_normally),
            lhs,
            32,
        );

        // If the rhs is subnormal, when we scale up then we don't need to scale down the result
        let mut if_rhs_subnormal_or_zero = naga::Block::default();
        {
            let rhs_scaled = scale_up_float(
                &mut (&mut *module, function_handle, &mut if_rhs_subnormal_or_zero),
                rhs,
                32,
            );

            let res = naga_expr!(module, function_handle, if_rhs_subnormal_or_zero => lhs_scaled / rhs_scaled);

            if_rhs_subnormal_or_zero.push_store(res_ptr, res);
        }

        let mut if_rhs_not_subnormal_or_zero = naga::Block::default();
        {
            let is_rhs_tiny = naga_expr!(module, function_handle => rhs_exp <= U32(64));

            let mut if_rhs_tiny = naga::Block::default();
            {
                let res_scaled =
                    naga_expr!(module, function_handle, if_rhs_tiny => lhs_scaled / rhs);

                let res = scale_down_float(
                    &mut (&mut *module, function_handle, &mut if_rhs_tiny),
                    res_scaled,
                    32,
                );

                if_rhs_tiny.push_store(res_ptr, res);
            }

            let mut if_rhs_not_tiny = naga::Block::default();
            {
                let rhs_scaled = scale_down_float(
                    &mut (&mut *module, function_handle, &mut if_rhs_not_tiny),
                    rhs,
                    32,
                );

                let res_scaled =
                    naga_expr!(module, function_handle, if_rhs_not_tiny => lhs_scaled / rhs_scaled);

                let res = scale_down_float(
                    &mut (&mut *module, function_handle, &mut if_rhs_not_tiny),
                    res_scaled,
                    64,
                );

                if_rhs_not_tiny.push_store(res_ptr, res);
            }

            if_rhs_not_subnormal_or_zero.push_if(is_rhs_tiny, if_rhs_tiny, if_rhs_not_tiny);
        }

        if_not_divide_normally.push_if(
            is_rhs_subnormal_or_zero,
            if_rhs_subnormal_or_zero,
            if_rhs_not_subnormal_or_zero,
        );
    }

    module.fn_mut(function_handle).body.push_if(
        can_divide_normally,
        if_divide_normally,
        if_not_divide_normally,
    );

    return naga_expr!(module, function_handle => Load(res_ptr));
}

fn subnormal_sqrt(
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    ty: naga::Handle<naga::Type>,
    value: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!(module, function_handle => sqrt(value));

    let res_var =
        module
            .fn_mut(function_handle)
            .local_variables
            .new_local("subnormal_res", ty, None);
    let res_ptr = module
        .fn_mut(function_handle)
        .expressions
        .append_local(res_var);
    module.fn_mut(function_handle).body.push_store(res_ptr, res);

    let is_subnormal = naga_expr!(module, function_handle => (res <= F32(9.861e-32)) & ((bitcast<u32>(value) | U32(0x80000000)) != U32(0x80000000)));

    let mut if_subnormal = naga::Block::default();
    {
        let value_scaled = scale_up_float(
            &mut (&mut *module, function_handle, &mut if_subnormal),
            value,
            64,
        );

        let res_scaled = naga_expr!(module, function_handle, if_subnormal => sqrt(value_scaled));

        let res = scale_down_float(
            &mut (&mut *module, function_handle, &mut if_subnormal),
            res_scaled,
            32,
        );

        if_subnormal.push_store(res_ptr, res);
    }

    module
        .fn_mut(function_handle)
        .body
        .push_if(is_subnormal, if_subnormal, naga::Block::default());

    return naga_expr!(module, function_handle => Load(res_ptr));
}

fn subnormal_min(
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!(module, function_handle => min(lhs, rhs));

    let res_var =
        module
            .fn_mut(function_handle)
            .local_variables
            .new_local("subnormal_res", ty, None);
    let res_ptr = module
        .fn_mut(function_handle)
        .expressions
        .append_local(res_var);
    module.fn_mut(function_handle).body.push_store(res_ptr, res);

    let is_subnormal = naga_expr!(module, function_handle => (res >= F32(-9.861e-32))
                                         & (res <= F32(9.861e-32))
    );

    let mut if_subnormal = naga::Block::default();
    {
        // Implement min manually
        let new_res = naga_expr!(module, function_handle, if_subnormal =>
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
        );

        if_subnormal.push_store(res_ptr, new_res);
    }

    module
        .fn_mut(function_handle)
        .body
        .push_if(is_subnormal, if_subnormal, naga::Block::default());

    return naga_expr!(module, function_handle => Load(res_ptr));
}

fn subnormal_max(
    module: &mut naga::Module,
    function_handle: naga::Handle<naga::Function>,
    ty: naga::Handle<naga::Type>,
    lhs: naga::Handle<naga::Expression>,
    rhs: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    // First approx.
    let res = naga_expr!(module, function_handle => max(lhs, rhs));

    let res_var =
        module
            .fn_mut(function_handle)
            .local_variables
            .new_local("subnormal_res", ty, None);
    let res_ptr = module
        .fn_mut(function_handle)
        .expressions
        .append_local(res_var);
    module.fn_mut(function_handle).body.push_store(res_ptr, res);

    let is_subnormal = naga_expr!(module, function_handle => (res >= F32(-9.861e-32))
                                         & (res <= F32(9.861e-32))
    );

    let mut if_subnormal = naga::Block::default();
    {
        // Implement min manually
        let new_res = naga_expr!(module, function_handle, if_subnormal =>
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
        );

        if_subnormal.push_store(res_ptr, new_res);
    }

    module
        .fn_mut(function_handle)
        .body
        .push_if(is_subnormal, if_subnormal, naga::Block::default());

    return naga_expr!(module, function_handle => Load(res_ptr));
}

/// An implementation of f32s using the GPU's native f32 type
pub(crate) struct NativeF32;
impl F32Gen for NativeF32 {
    fn gen_ty(
        module: &mut naga::Module,
        _others: super::f32_instance_gen::TyRequirements,
    ) -> build::Result<super::f32_instance_gen::Ty> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Float,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }

    fn gen_default(
        module: &mut naga::Module,
        _others: super::f32_instance_gen::DefaultRequirements,
    ) -> build::Result<super::f32_instance_gen::Default> {
        Ok(make_const_impl(&mut module.constants, 0.0))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _others: super::f32_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::f32_instance_gen::SizeBytes> {
        Ok(4)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        _others: super::f32_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::f32_instance_gen::MakeConst> {
        Ok(Arc::new(Box::new(|module, _, value| {
            Ok(make_const_impl(module, value))
        })))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        others: super::f32_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::f32_instance_gen::ReadInput> {
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
        others: super::f32_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::f32_instance_gen::WriteOutput> {
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
        others: super::f32_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::f32_instance_gen::ReadMemory> {
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
        others: super::f32_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::f32_instance_gen::WriteMemory> {
        gen_write(
            module,
            others.word,
            others.ty,
            others.bindings.memory,
            "memory",
        )
    }

    fn gen_abs(
        module: &mut naga::Module,
        others: f32_instance_gen::AbsRequirements,
    ) -> build::Result<f32_instance_gen::Abs> {
        let (function_handle, value) = declare_function! {
            module => fn f32_abs(value: others.ty) -> others.ty
        };

        // Just unset sign bit
        let res = naga_expr!(module, function_handle => bitcast<f32>(bitcast<u32>(value) & U32(0x7FFFFFFF)));

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_neg(
        module: &mut naga::Module,
        others: f32_instance_gen::NegRequirements,
    ) -> build::Result<f32_instance_gen::Neg> {
        let (function_handle, value) = declare_function! {
            module => fn f32_neg(value: others.ty) -> others.ty
        };

        // Just flip sign bit
        let res = naga_expr!(module, function_handle => bitcast<f32>(bitcast<u32>(value) ^ U32(0x80000000)));

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_ceil(
        module: &mut naga::Module,
        others: f32_instance_gen::CeilRequirements,
    ) -> build::Result<f32_instance_gen::Ceil> {
        let (function_handle, value) = declare_function! {
            module => fn f32_ceil(value: others.ty) -> others.ty
        };

        let res = if others.fp_options.emulate_subnormals {
            naga_expr!(module, function_handle =>
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
            naga_expr!(module, function_handle => ceil(value))
        };

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_floor(
        module: &mut naga::Module,
        others: f32_instance_gen::FloorRequirements,
    ) -> build::Result<f32_instance_gen::Floor> {
        let (function_handle, value) = declare_function! {
            module => fn f32_floor(value: others.ty) -> others.ty
        };

        let res = if others.fp_options.emulate_subnormals {
            naga_expr!(module, function_handle =>
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
            naga_expr!(module, function_handle => floor(value))
        };

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_trunc(
        module: &mut naga::Module,
        others: f32_instance_gen::TruncRequirements,
    ) -> build::Result<f32_instance_gen::Trunc> {
        let (function_handle, value) = declare_function! {
            module => fn f32_trunc(value: others.ty) -> others.ty
        };

        // No subnormal correction required because `trunc(±ε) == ±0.0`
        let res = naga_expr!(module, function_handle => trunc(value));

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_nearest(
        module: &mut naga::Module,
        others: f32_instance_gen::NearestRequirements,
    ) -> build::Result<f32_instance_gen::Nearest> {
        let (function_handle, value) = declare_function! {
            module => fn f32_nearest(value: others.ty) -> others.ty
        };

        // No subnormal correction required because `nearest(±ε) == ±0.0`
        let res = naga_expr!(module, function_handle => round(value));

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_sqrt(
        module: &mut naga::Module,
        others: f32_instance_gen::SqrtRequirements,
    ) -> build::Result<f32_instance_gen::Sqrt> {
        let (function_handle, value) = declare_function! {
            module => fn f32_sqrt(value: others.ty) -> others.ty
        };

        let res = if others.fp_options.emulate_subnormals {
            let subnormal_sqrt = subnormal_sqrt(module, function_handle, others.ty, value);
            naga_expr!(module, function_handle =>
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
            naga_expr!(module, function_handle => sqrt(value))
        };

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_min(
        module: &mut naga::Module,
        others: f32_instance_gen::MinRequirements,
    ) -> build::Result<f32_instance_gen::Min> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn f32_min(lhs: others.ty, rhs: others.ty) -> others.ty
        };

        let min = if others.fp_options.emulate_subnormals {
            subnormal_min(module, function_handle, others.ty, lhs, rhs)
        } else {
            naga_expr!(module, function_handle => min(lhs, rhs))
        };

        // Inbuilt `min` doesn't respect NaNs
        let is_nan = naga_expr!(module, function_handle => (((bitcast<u32>(lhs) & U32(0x7FFFFFFF)) > U32(0x7f800000)) | ((bitcast<u32>(rhs) & U32(0x7FFFFFFF)) > U32(0x7f800000))));
        let res = naga_expr!(module, function_handle => if (is_nan) {bitcast<f32>(U32(0x7fc00000))} else { min });

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_max(
        module: &mut naga::Module,
        others: f32_instance_gen::MaxRequirements,
    ) -> build::Result<f32_instance_gen::Max> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn f32_max(lhs: others.ty, rhs: others.ty) -> others.ty
        };

        let max = if others.fp_options.emulate_subnormals {
            subnormal_max(module, function_handle, others.ty, lhs, rhs)
        } else {
            naga_expr!(module, function_handle => max(lhs, rhs))
        };

        // Inbuilt `max` doesn't respect NaNs
        let is_nan = naga_expr!(module, function_handle => (((bitcast<u32>(lhs) & U32(0x7FFFFFFF)) > U32(0x7f800000)) | ((bitcast<u32>(rhs) & U32(0x7FFFFFFF)) > U32(0x7f800000))));
        let res = naga_expr!(module, function_handle => if (is_nan) {bitcast<f32>(U32(0x7fC00000))} else { max });

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_copy_sign(
        module: &mut naga::Module,
        others: f32_instance_gen::CopySignRequirements,
    ) -> build::Result<f32_instance_gen::CopySign> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn f32_copy_sign(lhs: others.ty, rhs: others.ty) -> others.ty
        };
        let res = naga_expr!(module, function_handle => sign(lhs) * sign(rhs) * lhs);
        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_add(
        module: &mut naga::Module,
        others: f32_instance_gen::AddRequirements,
    ) -> build::Result<f32_instance_gen::Add> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn f32_add(lhs:others.ty, rhs:others.ty) -> others.ty
        };

        let res = if others.fp_options.emulate_subnormals {
            subnormal_add(module, function_handle, others.ty, lhs, rhs)
        } else {
            naga_expr!(module, function_handle => lhs + rhs)
        };

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }
    fn gen_sub(
        module: &mut naga::Module,
        others: f32_instance_gen::SubRequirements,
    ) -> build::Result<f32_instance_gen::Sub> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn f32_sub(lhs:others.ty,rhs:others.ty)->others.ty
        };

        let res = if others.fp_options.emulate_subnormals {
            subnormal_sub(module, function_handle, others.ty, lhs, rhs)
        } else {
            naga_expr!(module, function_handle => lhs - rhs)
        };

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }
    fn gen_mul(
        module: &mut naga::Module,
        others: f32_instance_gen::MulRequirements,
    ) -> build::Result<f32_instance_gen::Mul> {
        let (function_handle, lhs, rhs) = declare_function! {
          module => fn f32_mul(lhs:others.ty,rhs:others.ty)->others.ty
        };

        let res = if others.fp_options.emulate_subnormals {
            subnormal_mult(module, function_handle, others.ty, lhs, rhs)
        } else {
            naga_expr!(module, function_handle => lhs * rhs)
        };

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    impl_native_bool_binexp! { f32_instance_gen, f32, eq; == }
    impl_native_bool_binexp! { f32_instance_gen, f32, ne; != }

    fn gen_div(
        module: &mut naga::Module,
        others: f32_instance_gen::DivRequirements,
    ) -> build::Result<f32_instance_gen::Div> {
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn f32_div(lhs:others.ty, rhs:others.ty) -> others.ty
        };

        if others.fp_options.emulate_div_beyond_max {
            let lhs_exp =
                naga_expr!(module, function_handle => (bitcast<u32>(lhs) >> U32(23)) & U32(0xFF));
            let rhs_exp =
                naga_expr!(module, function_handle => (bitcast<u32>(rhs) >> U32(23)) & U32(0xFF));
            let is_lhs_beyond_max = naga_expr!(module, function_handle => (lhs_exp >= U32(253)) & (lhs_exp != U32(255)));
            let is_rhs_beyond_max = naga_expr!(module, function_handle => (rhs_exp >= U32(253)) & (rhs_exp != U32(255)));
            let is_both_beyond_max =
                naga_expr!(module, function_handle => is_lhs_beyond_max & is_rhs_beyond_max);

            let mut if_both_beyond_max = naga::Block::default();
            {
                let lhs_scaled = scale_down_float(
                    &mut (&mut *module, function_handle, &mut if_both_beyond_max),
                    lhs,
                    32,
                );
                let rhs_scaled = scale_down_float(
                    &mut (&mut *module, function_handle, &mut if_both_beyond_max),
                    rhs,
                    32,
                );
                let res = naga_expr!(module, function_handle, if_both_beyond_max => lhs_scaled / rhs_scaled);
                if_both_beyond_max.push_return(res);
            }

            let mut if_lhs_beyond_max = naga::Block::default();
            {
                let lhs_scaled = scale_down_float(
                    &mut (&mut *module, function_handle, &mut if_lhs_beyond_max),
                    lhs,
                    32,
                );
                let res_scaled =
                    naga_expr!(module, function_handle, if_lhs_beyond_max => lhs_scaled / rhs);
                let res = scale_up_float(
                    &mut (&mut *module, function_handle, &mut if_lhs_beyond_max),
                    res_scaled,
                    32,
                );
                if_lhs_beyond_max.push_return(res);
            }

            let mut if_rhs_beyond_max = naga::Block::default();
            {
                let rhs_scaled = scale_down_float(
                    &mut (&mut *module, function_handle, &mut if_rhs_beyond_max),
                    rhs,
                    32,
                );
                let res_scaled =
                    naga_expr!(module, function_handle, if_rhs_beyond_max => lhs / rhs_scaled);
                let res = scale_down_float(
                    &mut (&mut *module, function_handle, &mut if_rhs_beyond_max),
                    res_scaled,
                    32,
                );
                if_rhs_beyond_max.push_return(res);
            }

            module.fn_mut(function_handle).body.push_if(
                is_both_beyond_max,
                if_both_beyond_max,
                naga::Block::default(),
            );
            module.fn_mut(function_handle).body.push_if(
                is_lhs_beyond_max,
                if_lhs_beyond_max,
                naga::Block::default(),
            );
            module.fn_mut(function_handle).body.push_if(
                is_rhs_beyond_max,
                if_rhs_beyond_max,
                naga::Block::default(),
            );
        }

        let res = if others.fp_options.emulate_subnormals {
            subnormal_div(module, function_handle, others.ty, lhs, rhs)
        } else {
            naga_expr!(module, function_handle => lhs / rhs)
        };

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    super::impl_load_and_store! {f32_instance_gen, f32}

    fn gen_convert_i32_s(
        module: &mut naga::Module,
        others: f32_instance_gen::ConvertI32SRequirements,
    ) -> build::Result<f32_instance_gen::ConvertI32S> {
        let (function_handle, value) = declare_function! {
            module => fn f32_convert_i32_s(value: others.i32_ty) -> others.ty
        };

        // No subnormal correction required because integers can't be subnormals
        let res = naga_expr!(module, function_handle => f32(value));

        module.fn_mut(function_handle).body.push_return(res);
        Ok(function_handle)
    }

    fn gen_convert_i32_u(
        module: &mut naga::Module,
        others: f32_instance_gen::ConvertI32URequirements,
    ) -> build::Result<f32_instance_gen::ConvertI32U> {
        let (function_handle, value) = declare_function! {
            module => fn f32_convert_i32_s(value: others.i32_ty) -> others.ty
        };

        // No subnormal correction required because integers can't be subnormals
        let res = naga_expr!(module, function_handle => f32(bitcast<u32>(value)));

        module.fn_mut(function_handle).body.push_return(res);
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
    address_ty: std_objects_gen::Word,
    f32_ty: f32_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_f32_from_{}", buffer_name);

    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> f32_ty
    };

    let read_word = naga_expr!(module, function_handle => Global(buffer)[word_address]);
    let read_value = naga_expr!(module, function_handle => Load(read_word) as Float);
    module.fn_mut(function_handle).body.push_return(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: f32)
fn gen_write(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    f32_ty: f32_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_f32_to_{}", buffer_name);
    let (function_handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: f32_ty)
    };

    let write_word_loc = naga_expr!(module, function_handle => Global(buffer)[word_address]);
    let word = naga_expr!(module, function_handle => value as Uint);
    module
        .fn_mut(function_handle)
        .body
        .push_store(write_word_loc, word);

    Ok(function_handle)
}
