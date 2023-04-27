use std::sync::Arc;

use wasm_types::{ExternRef, FuncRef, V128};

use crate::build;

use super::{bindings::StdBindings, StdObjects, WasmBoolInstance};

pub(crate) mod native_f32;
pub(crate) mod native_i32;
pub(crate) mod pollyfill_extern_ref;
pub(crate) mod pollyfill_func_ref;
pub(crate) mod polyfill_f64;
pub(crate) mod polyfill_i64;
pub(crate) mod polyfill_v128;

type MakeConstFn<Ty> = Arc<
    Box<
        dyn Fn(
            &mut naga::Arena<naga::Constant>,
            &StdObjects,
            Ty,
        ) -> build::Result<naga::Handle<naga::Constant>>,
    >,
>;

macro_rules! wasm_ty_generator {
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [$($parts:tt)*]) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts)*]; { }}
    };
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; []; {$($impl:tt)*}) => {
        super::generator_struct! {
            pub(crate) struct $struct_name (
                word: naga::Handle<naga::Type>,
                bindings: StdBindings,
                word_max: naga::Handle<naga::Constant>,
                bool: WasmBoolInstance,
            )
            {
                // Things all wasm types have
                ty: naga::Handle<naga::Type>,
                default: |ty| naga::Handle<naga::Constant>,

                size_bytes: u32,
                make_const: MakeConstFn<$wasm_ty>,

                read_input: |word, ty, bindings| naga::Handle<naga::Function>,
                write_output: |word, ty, bindings| naga::Handle<naga::Function>,
                read_memory: |word, ty, bindings| naga::Handle<naga::Function>,
                write_memory: |word, ty, bindings| naga::Handle<naga::Function>,

                $($impl)*
            } with pub(crate) trait $trait_name;
        }
    };
    // The implementation required for numerics (i32, i64, f32, f64)
    // See https://webassembly.github.io/spec/core/syntax/instructions.html#numeric-instructions
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [numeric $(, $parts:tt)*]; {$($impl:tt)*}) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts)*]; {
            $($impl)*

            add: |ty, word_max| naga::Handle<naga::Function>,

            eq: |ty, bool| naga::Handle<naga::Function>,
        }}
    };
    // The implementation required for integers (i32, i64)
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [integer $(, $parts:tt)*]; {$($impl:tt)*}) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts)*]; {
            $($impl)*

            eqz: |ty, bool| naga::Handle<naga::Function>,
        }}
    };
}

wasm_ty_generator!(struct I32Instance; trait I32Gen; i32; [numeric, integer]);
wasm_ty_generator!(struct I64Instance; trait I64Gen; i64; [numeric, integer]);
wasm_ty_generator!(struct F32Instance; trait F32Gen; f32; [numeric]);
wasm_ty_generator!(struct F64Instance; trait F64Gen; f64; [numeric]);
wasm_ty_generator!(struct V128Instance; trait V128Gen; V128; []);
wasm_ty_generator!(struct FuncRefInstance; trait FuncRefGen; FuncRef; []);
wasm_ty_generator!(struct ExternRefInstance; trait ExternRefGen; ExternRef; []);

fn make_64_bit_const_from_2vec32(
    ty: naga::Handle<naga::Type>,
    constants: &mut naga::Arena<naga::Constant>,
    value: i64,
) -> naga::Handle<naga::Constant> {
    let inner = naga::ConstantInner::Composite {
        ty: ty.clone(),
        components: (0..2)
            .map(|i_word| {
                let word = value >> (32 * i_word);
                let word =
                    u32::try_from(word & 0xFFFFFFFF).expect("truncated word always fits in u32");
                constants.append(
                    naga::Constant {
                        name: None,
                        specialization: None,
                        inner: naga::ConstantInner::Scalar {
                            width: 4,
                            value: naga::ScalarValue::Uint(word.into()),
                        },
                    },
                    naga::Span::UNDEFINED,
                )
            })
            .collect(),
    };
    constants.append(
        naga::Constant {
            name: None,
            specialization: None,
            inner,
        },
        naga::Span::UNDEFINED,
    )
}

macro_rules! impl_native_ops {
    ($instance_gen:ident, $name:ident) => {
        paste::paste! {
            fn gen_add(
                module: &mut naga::Module,
                others: $instance_gen::AddRequirements,
            ) -> build::Result<$instance_gen::Add> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _add >](lhs: others.ty, rhs: others.ty) -> others.ty
                };

                let res = naga_expr!(module, function_handle => lhs + rhs);
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }

            fn gen_eq(
                module: &mut naga::Module,
                others: $instance_gen::EqRequirements,
            ) -> build::Result<$instance_gen::Eq> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _eq >](lhs: others.ty, rhs: others.ty) -> others.bool.ty
                };

                let t = naga_expr!(module, function_handle => Constant(others.bool.const_true));
                let f = naga_expr!(module, function_handle => Constant(others.bool.const_false));
                let res = naga_expr!(module, function_handle => if (lhs == rhs) {t} else {f});
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_native_ops;

macro_rules! impl_integer_ops {
    ($instance_gen:ident, $name:ident) => {
        paste::paste! {
            /*fn gen_eqz(
                module: &mut naga::Module,
                others: $instance_gen::EqzRequirements,
            ) -> build::Result<$instance_gen::Eqz> {
                let (function_handle, value) = declare_function! {
                    module => fn [< $name _eqz >](value: others.ty) -> others.bool.ty
                };

                let zero = naga_expr!(module, function_handle => Constant(others.default));

                let res = module.fn_mut(function_handle).expressions.append(naga::Expression::CallResult(others.eq), naga::Span::UNDEFINED);

                module.fn_mut(function_handle).body.push(naga::Statement::Call { function: others.eq, arguments: vec![value, zero], result: Some(res) }, naga::Span::UNDEFINED);
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }*/
        }
    };
}
use impl_integer_ops;

macro_rules! impl_bitwise_2vec32_numeric_ops {
    ($instance_gen:ident, $name:ident) => {
        paste::paste!{
            fn gen_eq(
                module: &mut naga::Module,
                others: $instance_gen::EqRequirements,
            ) -> build::Result<$instance_gen::Eq> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _eq >](lhs: others.ty, rhs: others.ty) -> others.bool.ty
                };

                let t = naga_expr!(module, function_handle => Constant(others.bool.const_true));
                let f = naga_expr!(module, function_handle => Constant(others.bool.const_false));

                let lhs_high = naga_expr!(module, function_handle => lhs[const 0]);
                let lhs_low = naga_expr!(module, function_handle => lhs[const 1]);
                let rhs_high = naga_expr!(module, function_handle => rhs[const 0]);
                let rhs_low = naga_expr!(module, function_handle => rhs[const 1]);
                let res = naga_expr!(module, function_handle => if ((lhs_low == rhs_low) & (lhs_high == rhs_high)) {t} else {f});
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_bitwise_2vec32_numeric_ops;
