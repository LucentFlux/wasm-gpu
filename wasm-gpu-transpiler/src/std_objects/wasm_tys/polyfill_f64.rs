use super::{f64_instance_gen, F64Gen};
use crate::{build, std_objects::preamble_objects_gen};
use naga_ext::{declare_function, naga_expr, BlockContext, ConstantsExt, TypesExt};

#[derive(Clone)]
struct FrexpParts {
    sign: naga::Handle<naga::Expression>,
    exponent: naga::Handle<naga::Expression>,
    upper_magnitude: naga::Handle<naga::Expression>,
    lower_magnitude: naga::Handle<naga::Expression>,
}

impl FrexpParts {
    fn from_uvec2(ctx: &mut BlockContext<'_>, value: naga::Handle<naga::Expression>) -> Self {
        let word1 = naga_expr!(ctx => value[const 0]);
        let word2 = naga_expr!(ctx => value[const 1]);

        let sign = naga_expr!(ctx => word1 >> U32(31));
        let exponent = naga_expr!(ctx => (word1 >> U32(20)) & U32((1 << 11) - 1));
        let upper_magnitude = naga_expr!(ctx => word1 & U32((1 << 20) - 1));
        let lower_magnitude = naga_expr!(ctx => word2);

        Self {
            sign,
            exponent,
            upper_magnitude,
            lower_magnitude,
        }
    }

    /// Converts an f64 into its constituent parts
    fn from_f64(
        module: &mut naga::Module,
        function_handle: naga::Handle<naga::Function>,
        value: naga::Handle<naga::Expression>,
    ) -> Self {
        todo!()
    }

    /// Generates code that adds two floats
    fn gen_add(self, rhs_frexp: FrexpParts, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    /// Generates code that subtracts two floats
    fn gen_sub(self, rhs_frexp: FrexpParts, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    /// Generates code that multiplies two floats
    fn gen_mul(self, rhs_frexp: FrexpParts, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    /// Generates code that divides two floats
    fn gen_div(self, rhs_frexp: FrexpParts, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    /// Generates code that finds the min of two floats
    fn gen_min(self, rhs_frexp: FrexpParts, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    /// Generates code that finds the max of two floats
    fn gen_max(self, rhs_frexp: FrexpParts, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    /// Generates code that copies the sign of one float to another
    fn gen_copy_sign(self, rhs_frexp: FrexpParts, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    fn gen_abs(self, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    fn gen_neg(self, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    fn gen_ceil(self, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    fn gen_floor(self, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    fn gen_trunc(self, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    fn gen_nearest(self, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    fn gen_sqrt(self, ctx: &mut BlockContext<'_>) -> FrexpParts {
        self // TODO: This
    }

    fn gen_lt(
        self,
        rhs_frexp: FrexpParts,
        ctx: &mut BlockContext<'_>,
    ) -> naga::Handle<naga::Expression> {
        ctx.literal_expr_from(0) // TODO: This
    }

    fn gen_le(
        self,
        rhs_frexp: FrexpParts,
        ctx: &mut BlockContext<'_>,
    ) -> naga::Handle<naga::Expression> {
        ctx.literal_expr_from(0) // TODO: This
    }

    fn gen_gt(
        self,
        rhs_frexp: FrexpParts,
        ctx: &mut BlockContext<'_>,
    ) -> naga::Handle<naga::Expression> {
        ctx.literal_expr_from(0) // TODO: This
    }

    fn gen_ge(
        self,
        rhs_frexp: FrexpParts,
        ctx: &mut BlockContext<'_>,
    ) -> naga::Handle<naga::Expression> {
        ctx.literal_expr_from(0) // TODO: This
    }

    /// Combines all of the component expressions into a 64 bit float, possibly losing subnormals and handling inf/nans badly
    fn gen_f64(
        self,
        ctx: &mut BlockContext<'_>,
        f64_ty: naga::Handle<naga::Type>,
    ) -> naga::Handle<naga::Expression> {
        todo!();
        // This is entirely untested
        /*let exp_coeff = naga_expr!(ctx => exp2(self.exponent));
        let high_coeff = naga_expr!(ctx => F64(f64::exp2(-19.0)) * exp_coeff);
        let low_coeff = naga_expr!(ctx => F64(f64::exp2(-51.0)) * exp_coeff);

        let high_val =
            naga_expr!(ctx => f64_ty(self.upper_magnitude) * high_coeff);
        let low_val =
            naga_expr!(ctx => f64_ty(self.lower_magnitude) * low_coeff);
        naga_expr!(ctx => (high_val + low_val) * if ({self.sign} > U32(0)) {F64(-1.0)} else {F64(1.0)})*/
    }

    /// Combines all of the component expressions back into the underlying representation
    fn gen_uvec2(
        self,
        ctx: &mut BlockContext<'_>,
        uvec2_ty: naga::Handle<naga::Type>,
    ) -> naga::Handle<naga::Expression> {
        let res_high = naga_expr!(ctx => ({self.sign} << U32(31)) | ({self.exponent} << U32(20)) | {self.upper_magnitude});
        let res_low = naga_expr!(ctx => {self.lower_magnitude});
        naga_expr!(ctx => uvec2_ty(res_high, res_low))
    }
}

macro_rules! impl_mono_using_frexp {
    ($instance_gen:ident, $fn:ident) => {
        paste::paste! {
            fn [< gen_ $fn >](
                module: &mut naga::Module,
                requirements: $instance_gen::[< $fn:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $fn:camel >]> {
                let f64_ty = *requirements.ty;
                let (function_handle, value) = declare_function! {
                    module => fn [< f64_ $fn >](value: f64_ty) -> f64_ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let value_frexp = FrexpParts::from_uvec2(&mut ctx, value);

                let res_frexp = value_frexp.[< gen_ $fn >](&mut ctx);

                let res = res_frexp.gen_uvec2(&mut ctx, f64_ty);
                ctx.result(res);

                Ok(function_handle)
            }
        }
    };
}

macro_rules! impl_binary_using_frexp {
    ($instance_gen:ident, $fn:ident) => {
        paste::paste! {
            fn [< gen_ $fn >](
                module: &mut naga::Module,
                requirements: $instance_gen::[< $fn:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $fn:camel >]> {
                let f64_ty = *requirements.ty;
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< f64_ $fn >](lhs: f64_ty, rhs: f64_ty) -> f64_ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let lhs_frexp = FrexpParts::from_uvec2(&mut ctx, lhs);
                let rhs_frexp = FrexpParts::from_uvec2(&mut ctx, rhs);

                let res_frexp = lhs_frexp.[< gen_ $fn >](rhs_frexp, &mut ctx);

                let res = res_frexp.gen_uvec2(&mut ctx, f64_ty);
                ctx.result(res);

                Ok(function_handle)
            }
        }
    };
}

macro_rules! impl_bool_binary_using_frexp {
    ($instance_gen:ident, $fn:ident) => {
        paste::paste! {
            fn [< gen_ $fn >](
                module: &mut naga::Module,
                requirements: $instance_gen::[< $fn:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $fn:camel >]> {
                let f64_ty = *requirements.ty;
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< f64_ $fn >](lhs: f64_ty, rhs: f64_ty) -> requirements.preamble.wasm_bool.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let lhs_frexp = FrexpParts::from_uvec2(&mut ctx, lhs);
                let rhs_frexp = FrexpParts::from_uvec2(&mut ctx, rhs);

                let res = lhs_frexp.[< gen_ $fn >](rhs_frexp, &mut ctx);
                ctx.result(res);

                Ok(function_handle)
            }
        }
    };
}

/// An implementation of f64s using a 2-vector of u32s
pub(crate) struct PolyfillF64;
impl F64Gen for PolyfillF64 {
    fn gen_ty(
        module: &mut naga::Module,
        _requirements: f64_instance_gen::TyRequirements,
    ) -> build::Result<f64_instance_gen::Ty> {
        let ty = module.types.insert_anonymous(naga::TypeInner::Vector {
            size: naga::VectorSize::Bi,
            scalar: naga::Scalar::U32,
        });

        Ok(ty)
    }

    fn gen_default(
        module: &mut naga::Module,
        requirements: f64_instance_gen::DefaultRequirements,
    ) -> build::Result<f64_instance_gen::Default> {
        let value = i64::from_le_bytes(f64::to_le_bytes(0.0));
        let init = super::make_64_bit_const_expr_from_2vec32(
            *requirements.ty,
            &mut module.const_expressions,
            value,
        );
        Ok(module.constants.append_anonymous(*requirements.ty, init))
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _requirements: f64_instance_gen::SizeBytesRequirements,
    ) -> build::Result<f64_instance_gen::SizeBytes> {
        Ok(8)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        requirements: f64_instance_gen::MakeConstRequirements,
    ) -> build::Result<f64_instance_gen::MakeConst> {
        let ty = *requirements.ty;
        Ok(Box::new(move |const_expressions, value| {
            let value = i64::from_le_bytes(value.to_le_bytes());
            let expr = super::make_64_bit_const_expr_from_2vec32(ty, const_expressions, value);
            Ok(expr)
        }))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        requirements: f64_instance_gen::ReadInputRequirements,
    ) -> build::Result<f64_instance_gen::ReadInput> {
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
        requirements: f64_instance_gen::WriteOutputRequirements,
    ) -> build::Result<f64_instance_gen::WriteOutput> {
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
        requirements: f64_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<f64_instance_gen::ReadMemory> {
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
        requirements: f64_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<f64_instance_gen::WriteMemory> {
        gen_write(
            module,
            requirements.preamble.word_ty,
            *requirements.ty,
            requirements.preamble.bindings.memory,
            "memory",
        )
    }

    impl_binary_using_frexp! {f64_instance_gen, add}
    impl_binary_using_frexp! {f64_instance_gen, sub}
    impl_binary_using_frexp! {f64_instance_gen, mul}
    impl_binary_using_frexp! {f64_instance_gen, div}
    impl_binary_using_frexp! {f64_instance_gen, min}
    impl_binary_using_frexp! {f64_instance_gen, max}
    impl_binary_using_frexp! {f64_instance_gen, copy_sign}

    super::impl_bitwise_2vec32_numeric_ops! {f64_instance_gen, f64}

    super::impl_load_and_store! {f64_instance_gen, f64}

    impl_mono_using_frexp! {f64_instance_gen, abs}
    impl_mono_using_frexp! {f64_instance_gen, neg}
    impl_mono_using_frexp! {f64_instance_gen, ceil}
    impl_mono_using_frexp! {f64_instance_gen, floor}
    impl_mono_using_frexp! {f64_instance_gen, trunc}
    impl_mono_using_frexp! {f64_instance_gen, nearest}
    impl_mono_using_frexp! {f64_instance_gen, sqrt}

    impl_bool_binary_using_frexp! {f64_instance_gen, lt}
    impl_bool_binary_using_frexp! {f64_instance_gen, le}
    impl_bool_binary_using_frexp! {f64_instance_gen, gt}
    impl_bool_binary_using_frexp! {f64_instance_gen, ge}
}

// fn<buffer>(word_address: u32) -> f64
fn gen_read(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    f64_ty: f64_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_f64_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> f64_ty
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let input_ref = naga_expr!(&mut ctx => Global(buffer));

    let read_word1 = naga_expr!(&mut ctx => input_ref[word_address]);
    let read_word2 = naga_expr!(&mut ctx => input_ref[word_address + U32(1)]);
    let read_value = naga_expr!(&mut ctx => f64_ty(Load(read_word1), Load(read_word2)));
    ctx.result(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: f64)
fn gen_write(
    module: &mut naga::Module,
    address_ty: preamble_objects_gen::WordTy,
    f64_ty: f64_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_f64_to_{}", buffer_name);
    let (function_handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: f64_ty)
    };
    let mut ctx = BlockContext::from((module, function_handle));

    let output_ref = naga_expr!(&mut ctx => Global(buffer));

    let write_word_loc1 = naga_expr!(&mut ctx => output_ref[word_address]);
    let word1 = naga_expr!(&mut ctx => value[const 0]);
    let write_word_loc2 = naga_expr!(&mut ctx => output_ref[word_address + U32(1)]);
    let word2 = naga_expr!(&mut ctx => value[const 1]);

    ctx.store(write_word_loc1, word1);
    ctx.store(write_word_loc2, word2);

    Ok(function_handle)
}
