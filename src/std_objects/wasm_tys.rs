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
                wasm_bool: WasmBoolInstance,
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
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts),*]; {
            $($impl)*

            load:  |ty, word, read_memory|  naga::Handle<naga::Function>,
            store: |ty, word, write_memory| naga::Handle<naga::Function>,

            add: |ty, word_max| naga::Handle<naga::Function>,
            sub: |ty, word_max| naga::Handle<naga::Function>,
            mul: |ty, word_max| naga::Handle<naga::Function>,

            eq: |ty, wasm_bool| naga::Handle<naga::Function>,
            ne: |ty, wasm_bool| naga::Handle<naga::Function>,
        }}
    };
    // The implementation required for integers (i32, i64)
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [integer $(, $parts:tt)*]; {$($impl:tt)*}) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts),*]; {
            $($impl)*

            load_8_u:  |ty, default, word, read_memory|  naga::Handle<naga::Function>,
            load_8_s:  |ty, default, word, read_memory|  naga::Handle<naga::Function>,
            load_16_u: |ty, default, word, read_memory|  naga::Handle<naga::Function>,
            load_16_s: |ty, default, word, read_memory|  naga::Handle<naga::Function>,
            store_8:   |ty, default, word, write_memory| naga::Handle<naga::Function>,
            store_16:  |ty, default, word, write_memory| naga::Handle<naga::Function>,

            eqz: |ty, wasm_bool| naga::Handle<naga::Function>,

            lt_s: |ty, wasm_bool| naga::Handle<naga::Function>,
            le_s: |ty, wasm_bool| naga::Handle<naga::Function>,
            gt_s: |ty, wasm_bool| naga::Handle<naga::Function>,
            ge_s: |ty, wasm_bool| naga::Handle<naga::Function>,

            lt_u: |ty, wasm_bool| naga::Handle<naga::Function>,
            le_u: |ty, wasm_bool| naga::Handle<naga::Function>,
            gt_u: |ty, wasm_bool| naga::Handle<naga::Function>,
            ge_u: |ty, wasm_bool| naga::Handle<naga::Function>,

            // Atomics (from thread proposal)
            atomic_load:             |ty, default, word| naga::Handle<naga::Function>,
            atomic_load_8_u:         |ty, default, word| naga::Handle<naga::Function>,
            atomic_load_16_u:        |ty, default, word| naga::Handle<naga::Function>,
            atomic_store:            |ty, default, word| naga::Handle<naga::Function>,
            atomic_store_8:          |ty, default, word| naga::Handle<naga::Function>,
            atomic_store_16:         |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_add:          |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_8_add_u:      |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_16_add_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_sub:          |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_8_sub_u:      |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_16_sub_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_and:          |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_8_and_u:      |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_16_and_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_or:           |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_8_or_u:       |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_16_or_u:      |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_xor:          |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_8_xor_u:      |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_16_xor_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_xchg:         |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_8_xchg_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_16_xchg_u:    |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_cmpxchg:      |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_8_cmpxchg_u:  |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_16_cmpxchg_u: |ty, default, word| naga::Handle<naga::Function>,
        }}
    };
    // Just i64
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [i64 $(, $parts:tt)*]; {$($impl:tt)*}) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts),*]; {
            $($impl)*

            load_32_u:  |ty, default, word, read_memory|  naga::Handle<naga::Function>,
            load_32_s:  |ty, default, word, read_memory|  naga::Handle<naga::Function>,
            store_32:   |ty, default, word, write_memory| naga::Handle<naga::Function>,

            // Atomics (from thread proposal)
            atomic_load_32_u:        |ty, default, word| naga::Handle<naga::Function>,
            atomic_store_32:         |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_add_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_sub_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_and_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_or_u:      |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_xor_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_xchg_u:    |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_cmpxchg_u: |ty, default, word| naga::Handle<naga::Function>,
        }}
    };
    // The implementation required for floats (f32, f64)
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [floating $(, $parts:tt)*]; {$($impl:tt)*}) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts),*]; {
            $($impl)*

            div: |ty, word_max| naga::Handle<naga::Function>,
        }}
    };
}

wasm_ty_generator!(struct I32Instance; trait I32Gen; i32; [numeric, integer]);
wasm_ty_generator!(struct I64Instance; trait I64Gen; i64; [numeric, integer, i64]);
wasm_ty_generator!(struct F32Instance; trait F32Gen; f32; [numeric, floating]);
wasm_ty_generator!(struct F64Instance; trait F64Gen; f64; [numeric, floating]);
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

/// Something of the form `f(A, A) -> Bool` which can be implemented with native functions
macro_rules! impl_native_bool_binexp {
    ($instance_gen:ident, $name:ident, $op_name:ident; $op:tt) => {
        paste::paste! {
            fn [< gen_ $op_name >](
                module: &mut naga::Module,
                others: $instance_gen::[< $op_name:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $op_name:camel >]> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name $op_name >](lhs: others.ty, rhs: others.ty) -> others.wasm_bool.ty
                };

                let t = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_true));
                let f = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_false));
                let res = naga_expr!(module, function_handle => if (lhs $op rhs) {t} else {f});
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_native_bool_binexp;

/// Something of the form `f(A, A) -> A` which can be implemented with native functions
macro_rules! impl_native_inner_binexp {
    ($instance_gen:ident, $name:ident, $op_name:ident; $op:tt) => {
        paste::paste! {
            fn [< gen_ $op_name >](
                module: &mut naga::Module,
                others: $instance_gen::[< $op_name:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $op_name:camel >]> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name $op_name >](lhs: others.ty, rhs: others.ty) -> others.ty
                };

                let res = naga_expr!(module, function_handle => lhs $op rhs);
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_native_inner_binexp;

macro_rules! impl_native_unsigned_bool_binexp {
    ($instance_gen:ident, $name:ident, $op_name:ident; $op:tt) => {
        paste::paste! {
            fn [< gen_ $op_name >](
                module: &mut naga::Module,
                others: $instance_gen::[< $op_name:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $op_name:camel >]> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name $op_name >](lhs: others.ty, rhs: others.ty) -> others.wasm_bool.ty
                };

                let t = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_true));
                let f = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_false));
                let res = naga_expr!(module, function_handle => if ((lhs as Uint) $op (rhs as Uint)) {t} else {f});
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_native_unsigned_bool_binexp;

macro_rules! impl_native_ops {
    ($instance_gen:ident, $name:ident) => {
        paste::paste! {
            $crate::std_objects::wasm_tys::impl_native_inner_binexp!{$instance_gen, $name, add; +}
            $crate::std_objects::wasm_tys::impl_native_inner_binexp!{$instance_gen, $name, sub; -}
            $crate::std_objects::wasm_tys::impl_native_inner_binexp!{$instance_gen, $name, mul; *}

            $crate::std_objects::wasm_tys::impl_native_bool_binexp!{$instance_gen, $name, eq; ==}
            $crate::std_objects::wasm_tys::impl_native_bool_binexp!{$instance_gen, $name, ne; !=}
        }
    };
}
use impl_native_ops;

macro_rules! impl_bitwise_2vec32_numeric_ops {
    ($instance_gen:ident, $name:ident) => {
        paste::paste!{
            fn gen_eq(
                module: &mut naga::Module,
                others: $instance_gen::EqRequirements,
            ) -> build::Result<$instance_gen::Eq> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _eq >](lhs: others.ty, rhs: others.ty) -> others.wasm_bool.ty
                };

                let t = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_true));
                let f = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_false));

                let lhs_high = naga_expr!(module, function_handle => lhs[const 0]);
                let lhs_low = naga_expr!(module, function_handle => lhs[const 1]);
                let rhs_high = naga_expr!(module, function_handle => rhs[const 0]);
                let rhs_low = naga_expr!(module, function_handle => rhs[const 1]);
                let res = naga_expr!(module, function_handle => if ((lhs_low == rhs_low) & (lhs_high == rhs_high)) {t} else {f});
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }

            fn gen_ne(
                module: &mut naga::Module,
                others: $instance_gen::NeRequirements,
            ) -> build::Result<$instance_gen::Ne> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _ne >](lhs: others.ty, rhs: others.ty) -> others.wasm_bool.ty
                };

                let t = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_true));
                let f = naga_expr!(module, function_handle => Constant(others.wasm_bool.const_false));

                let lhs_high = naga_expr!(module, function_handle => lhs[const 0]);
                let lhs_low = naga_expr!(module, function_handle => lhs[const 1]);
                let rhs_high = naga_expr!(module, function_handle => rhs[const 0]);
                let rhs_low = naga_expr!(module, function_handle => rhs[const 1]);
                let res = naga_expr!(module, function_handle => if ((lhs_low != rhs_low) | (lhs_high != rhs_high)) {t} else {f});
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_bitwise_2vec32_numeric_ops;

macro_rules! impl_load_and_store {
    ($instance_gen:ident, $name:ident) => {
        paste::paste!{
            fn gen_load(
                module: &mut naga::Module,
                others: $instance_gen::LoadRequirements,
            ) -> build::Result<$instance_gen::Load> {
                let (function_handle, memory, address) = declare_function! {
                    module => fn [< $name _load >](memory: others.word, address: others.word) -> others.ty
                };

                // TODO: Support other memories
                drop(memory);

                // Variable to unify aligned and unaligned loads
                let loaded_value_local = module.fn_mut(function_handle).local_variables.new_local(
                    "loaded_value".to_owned(),
                    others.ty,
                    None,
                );
                let loaded_value_ptr = module
                    .fn_mut(function_handle)
                    .expressions
                    .append_local(loaded_value_local);

                // Load aligned
                let mut aligned_block = naga::Block::default();
                let aligned_address = naga_expr!(module, function_handle => address >> U32(2));
                let load_fn = others.read_memory;
                let load_result = module
                    .fn_mut(function_handle)
                    .expressions
                    .append(naga::Expression::CallResult(load_fn), naga::Span::UNDEFINED);
                aligned_block.push(
                    naga::Statement::Call {
                        function: load_fn,
                        arguments: vec![aligned_address],
                        result: Some(load_result),
                    },
                    naga::Span::UNDEFINED,
                );
                aligned_block.push_store(loaded_value_ptr, load_result);

                // We can do things much faster if the address is word-alligned
                // Otherwise we have to load twice and merge
                let mut unaligned_block = naga::Block::default();

                // TODO: unaligned path
                let aligned_address = naga_expr!(module, function_handle => address >> U32(2));
                let load_fn = others.read_memory;
                let load_result = module
                    .fn_mut(function_handle)
                    .expressions
                    .append(naga::Expression::CallResult(load_fn), naga::Span::UNDEFINED);
                unaligned_block.push(
                    naga::Statement::Call {
                        function: load_fn,
                        arguments: vec![aligned_address],
                        result: Some(load_result),
                    },
                    naga::Span::UNDEFINED,
                );
                unaligned_block.push_store(loaded_value_ptr, load_result);
                // END TODO

                // Test for unalignment
                let unaligned_condition =
                    naga_expr!(module, function_handle => (address & U32(3)) != U32(0));
                module.fn_mut(function_handle).body.push_if(unaligned_condition,
                    unaligned_block,
                    aligned_block
                );

                // Then unify load paths
                let res = naga_expr!(module, function_handle => Load(loaded_value_ptr));
                module.fn_mut(function_handle).body.push_return(res);

                Ok(function_handle)
            }

            fn gen_store(
                module: &mut naga::Module,
                others: $instance_gen::StoreRequirements,
            ) -> build::Result<$instance_gen::Store> {
                let (function_handle, memory, address, value) = declare_function! {
                    module => fn [< $name _store >](memory: others.word, address: others.word, value: others.ty)
                };

                // TODO: Support other memories
                drop(memory);

                // Store aligned
                let mut aligned_block = naga::Block::default();
                let aligned_address = naga_expr!(module, function_handle => address >> U32(2));
                let store_fn = others.write_memory;
                aligned_block.push(
                    naga::Statement::Call {
                        function: store_fn,
                        arguments: vec![aligned_address, value],
                        result: None,
                    },
                    naga::Span::UNDEFINED,
                );

                // We can do things much faster if the address is word-alligned
                // Otherwise we have to load twice, merge in value, and store back
                let mut unaligned_block = naga::Block::default();

                // TODO: unaligned path
                let aligned_address = naga_expr!(module, function_handle => address >> U32(2));
                let store_fn = others.write_memory;
                unaligned_block.push(
                    naga::Statement::Call {
                        function: store_fn,
                        arguments: vec![aligned_address, value],
                        result: None,
                    },
                    naga::Span::UNDEFINED,
                );
                // END TODO

                // Test for unalignment
                let unaligned_condition =
                    naga_expr!(module, function_handle => (address & U32(3)) != U32(0));
                module.fn_mut(function_handle).body.push_if(unaligned_condition,
                    unaligned_block,
                    aligned_block,
                );

                Ok(function_handle)
            }
        }
    };
}
use impl_load_and_store;

macro_rules! impl_dud_integer_load {
    ($instance_gen:ident, $name:ident, $fn:ident) => {
        paste::paste!{
            fn [< gen_ $fn >](
                module: &mut naga::Module,
                others: $instance_gen::[< $fn:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $fn:camel >]> {
                let (function_handle, memory, address) = declare_function! {
                    module => fn [< $name _ $fn >](memory: others.word, address: others.word) -> others.ty
                };
                let default = module.fn_mut(function_handle).expressions.append_constant(others.default);
                module.fn_mut(function_handle).body.push_return(default);

                Ok(function_handle)
            }
        }
    };
}
use impl_dud_integer_load;

macro_rules! impl_dud_integer_store {
    ($instance_gen:ident, $name:ident, $fn:ident) => {
        paste::paste!{
            fn [< gen_ $fn >](
                module: &mut naga::Module,
                others: $instance_gen::[< $fn:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $fn:camel >]> {
                let (function_handle, memory, address, value) = declare_function! {
                    module => fn [< $name _ $fn >](memory: others.word, address: others.word, value: others.ty)
                };

                Ok(function_handle)
            }
        }
    };
}
use impl_dud_integer_store;

macro_rules! impl_dud_integer_rmw {
    ($instance_gen:ident, $name:ident, $fn:ident) => {
        paste::paste!{
            fn [< gen_ $fn >](
                module: &mut naga::Module,
                others: $instance_gen::[< $fn:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $fn:camel >]> {
                let (function_handle, memory, address, value) = declare_function! {
                    module => fn [< $name _ $fn >](memory: others.word, address: others.word, operand: others.ty)
                };

                Ok(function_handle)
            }
        }
    };
}
use impl_dud_integer_rmw;

macro_rules! impl_integer_loads_and_stores {
    ($instance_gen:ident, $name:ident) => {
        paste::paste! {
            $crate::std_objects::wasm_tys::impl_dud_integer_load!{$instance_gen, $name, load_8_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_load!{$instance_gen, $name, load_8_s}
            $crate::std_objects::wasm_tys::impl_dud_integer_load!{$instance_gen, $name, load_16_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_load!{$instance_gen, $name, load_16_s}
            $crate::std_objects::wasm_tys::impl_dud_integer_store!{$instance_gen, $name, store_8}
            $crate::std_objects::wasm_tys::impl_dud_integer_store!{$instance_gen, $name, store_16}
        }
    };
}
use impl_integer_loads_and_stores;

macro_rules! impl_integer_atomic_loads_and_stores {
    ($instance_gen:ident, $name:ident) => {
        paste::paste!{
            $crate::std_objects::wasm_tys::impl_dud_integer_load!{$instance_gen, $name, atomic_load}
            $crate::std_objects::wasm_tys::impl_dud_integer_load!{$instance_gen, $name, atomic_load_8_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_load!{$instance_gen, $name, atomic_load_16_u}

            $crate::std_objects::wasm_tys::impl_dud_integer_store!{$instance_gen, $name, atomic_store}
            $crate::std_objects::wasm_tys::impl_dud_integer_store!{$instance_gen, $name, atomic_store_8}
            $crate::std_objects::wasm_tys::impl_dud_integer_store!{$instance_gen, $name, atomic_store_16}

            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_add}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_8_add_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_16_add_u}

            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_sub}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_8_sub_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_16_sub_u}

            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_and}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_8_and_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_16_and_u}

            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_or}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_8_or_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_16_or_u}

            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_xor}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_8_xor_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_16_xor_u}

            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_xchg}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_8_xchg_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_16_xchg_u}

            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_cmpxchg}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_8_cmpxchg_u}
            $crate::std_objects::wasm_tys::impl_dud_integer_rmw!{$instance_gen, $name, atomic_rmw_16_cmpxchg_u}
        }
    };
}
use impl_integer_atomic_loads_and_stores;
