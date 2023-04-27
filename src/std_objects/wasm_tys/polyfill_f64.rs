use std::sync::Arc;

use super::{f64_instance_gen, F64Gen};
use crate::{build, std_objects::std_objects_gen};
use naga_ext::{declare_function, naga_expr, BlockExt, ModuleExt};

fn make_const_impl(
    constants: &mut naga::Arena<naga::Constant>,
    ty: naga::Handle<naga::Type>,
    value: f64,
) -> build::Result<naga::Handle<naga::Constant>> {
    let value = i64::from_le_bytes(value.to_le_bytes());
    return Ok(super::make_64_bit_const_from_2vec32(ty, constants, value));
}

#[derive(Clone, Copy)]
struct FrexpParts {
    sign: naga::Handle<naga::Expression>,
    exponent: naga::Handle<naga::Expression>,
    upper_magnitude: naga::Handle<naga::Expression>,
    lower_magnitude: naga::Handle<naga::Expression>,
}

impl FrexpParts {
    fn from_uvec2(
        module: &mut naga::Module,
        function_handle: naga::Handle<naga::Function>,
        value: naga::Handle<naga::Expression>,
    ) -> Self {
        let word1 = naga_expr!(module, function_handle => value[const 0]);
        let word2 = naga_expr!(module, function_handle => value[const 1]);

        let sign = naga_expr!(module, function_handle => word1 >> U32(31));
        let exponent =
            naga_expr!(module, function_handle => (word1 >> U32(20)) & U32((1 << 11) - 1));
        let upper_magnitude = naga_expr!(module, function_handle => word1 & U32((1 << 20) - 1));
        let lower_magnitude = naga_expr!(module, function_handle => word2);

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

    /// Adds two floats
    fn gen_add(
        self,
        rhs_frexp: FrexpParts,
        module: &mut naga::Module,
        function_handle: naga::Handle<naga::Function>,
    ) -> FrexpParts {
        self // TODO: This
    }

    /// Combines all of the component expressions into a 64 bit float, possibly losing subnormals and handling inf/nans badly
    fn gen_f64(
        self,
        module: &mut naga::Module,
        function_handle: naga::Handle<naga::Function>,
        f64_ty: naga::Handle<naga::Type>,
    ) -> naga::Handle<naga::Expression> {
        // This is entirely untested
        todo!();
        let exp_coeff = naga_expr!(module, function_handle => exp2(self.exponent));
        let high_coeff = naga_expr!(module, function_handle => F64(f64::exp2(-19.0)) * exp_coeff);
        let low_coeff = naga_expr!(module, function_handle => F64(f64::exp2(-51.0)) * exp_coeff);

        let high_val =
            naga_expr!(module, function_handle => f64_ty(self.upper_magnitude) * high_coeff);
        let low_val =
            naga_expr!(module, function_handle => f64_ty(self.lower_magnitude) * low_coeff);
        naga_expr!(module, function_handle => (high_val + low_val) * if ({self.sign} > U32(0)) {F64(-1.0)} else {F64(1.0)})
    }

    /// Combines all of the component expressions back into the underlying representation
    fn gen_uvec2(
        self,
        module: &mut naga::Module,
        function_handle: naga::Handle<naga::Function>,
        uvec2_ty: naga::Handle<naga::Type>,
    ) -> naga::Handle<naga::Expression> {
        let res_high = naga_expr!(module, function_handle => ({self.sign} << U32(31)) | ({self.exponent} << U32(20)) | {self.upper_magnitude});
        let res_low = naga_expr!(module, function_handle => {self.lower_magnitude});
        naga_expr!(module, function_handle => uvec2_ty(res_high, res_low))
    }
}

/// An implementation of f64s using a 2-vector of u32s
pub(crate) struct PolyfillF64;
impl F64Gen for PolyfillF64 {
    fn gen_ty(
        module: &mut naga::Module,
        _others: super::f64_instance_gen::TyRequirements,
    ) -> build::Result<super::f64_instance_gen::Ty> {
        let naga_ty = naga::Type {
            name: Some("f64".to_owned()),
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Bi,
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        return Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED));
    }

    fn gen_default(
        module: &mut naga::Module,
        others: super::f64_instance_gen::DefaultRequirements,
    ) -> build::Result<super::f64_instance_gen::Default> {
        make_const_impl(&mut module.constants, others.ty, 0.0)
    }

    fn gen_size_bytes(
        _module: &mut naga::Module,
        _others: super::f64_instance_gen::SizeBytesRequirements,
    ) -> build::Result<super::f64_instance_gen::SizeBytes> {
        Ok(8)
    }

    fn gen_make_const(
        _module: &mut naga::Module,
        _others: super::f64_instance_gen::MakeConstRequirements,
    ) -> build::Result<super::f64_instance_gen::MakeConst> {
        Ok(Arc::new(Box::new(|module, std_objects, value| {
            make_const_impl(module, std_objects.f64.ty, value)
        })))
    }

    fn gen_read_input(
        module: &mut naga::Module,
        others: super::f64_instance_gen::ReadInputRequirements,
    ) -> build::Result<super::f64_instance_gen::ReadInput> {
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
        others: super::f64_instance_gen::WriteOutputRequirements,
    ) -> build::Result<super::f64_instance_gen::WriteOutput> {
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
        others: super::f64_instance_gen::ReadMemoryRequirements,
    ) -> build::Result<super::f64_instance_gen::ReadMemory> {
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
        others: super::f64_instance_gen::WriteMemoryRequirements,
    ) -> build::Result<super::f64_instance_gen::WriteMemory> {
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
        others: super::f64_instance_gen::AddRequirements,
    ) -> build::Result<super::f64_instance_gen::Add> {
        let f64_ty = others.ty;
        let (function_handle, lhs, rhs) = declare_function! {
            module => fn f64_add(lhs: f64_ty, rhs: f64_ty) -> f64_ty
        };

        let lhs_frexp = FrexpParts::from_uvec2(module, function_handle, lhs);
        let rhs_frexp = FrexpParts::from_uvec2(module, function_handle, rhs);

        let res_frexp = lhs_frexp.gen_add(rhs_frexp, module, function_handle);

        let res = res_frexp.gen_uvec2(module, function_handle, f64_ty);
        module.fn_mut(function_handle).body.push_return(res);

        Ok(function_handle)
    }

    super::impl_bitwise_2vec32_numeric_ops! {f64_instance_gen, f64}
}

// fn<buffer>(word_address: u32) -> f64
fn gen_read(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    f64_ty: f64_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("read_f64_from_{}", buffer_name);
    let (function_handle, word_address) = declare_function! {
        module => fn {fn_name}(word_address: address_ty) -> f64_ty
    };

    let input_ref = naga_expr!(module, function_handle => Global(buffer));

    let read_word1 = naga_expr!(module, function_handle => input_ref[word_address]);
    let read_word2 = naga_expr!(module, function_handle => input_ref[word_address + U32(1)]);
    let read_value =
        naga_expr!(module, function_handle => f64_ty(Load(read_word1), Load(read_word2)));
    module.fn_mut(function_handle).body.push_return(read_value);

    Ok(function_handle)
}

// fn<buffer>(word_address: u32, value: f64)
fn gen_write(
    module: &mut naga::Module,
    address_ty: std_objects_gen::Word,
    f64_ty: f64_instance_gen::Ty,
    buffer: naga::Handle<naga::GlobalVariable>,
    buffer_name: &str,
) -> build::Result<naga::Handle<naga::Function>> {
    let fn_name = format!("write_f64_to_{}", buffer_name);
    let (handle, word_address, value) = declare_function! {
        module => fn {fn_name}(word_address: address_ty, value: f64_ty)
    };

    let output_ref = naga_expr!(module, handle => Global(buffer));

    let write_word_loc1 = naga_expr!(module, handle => output_ref[word_address]);
    let word1 = naga_expr!(module, handle => value[const 0]);
    let write_word_loc2 = naga_expr!(module, handle => output_ref[word_address + U32(1)]);
    let word2 = naga_expr!(module, handle => value[const 1]);

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
