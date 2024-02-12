use crate::build;
use crate::typed::{ExternRef, FuncRef, V128};

pub(crate) mod native_f32;
pub(crate) mod native_i32;
pub(crate) mod pollyfill_extern_ref;
pub(crate) mod pollyfill_func_ref;
pub(crate) mod polyfill_f64;
pub(crate) mod polyfill_i64;
pub(crate) mod polyfill_v128;

type MakeConstFn<Ty> = Box<
    dyn Fn(&mut naga::Arena<naga::Expression>, Ty) -> build::Result<naga::Handle<naga::Expression>>,
>;

macro_rules! wasm_ty_generator {
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [$($parts:tt)*]) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts)*]; { }; ()}
    };
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; []; {$($impl:tt)*}; ($($extra_params:tt)*)) => {
        super::generator_struct! {
            pub(crate) struct $struct_name (
                preamble: crate::std_objects::PreambleObjects,
                fp_options: crate::FloatingPointOptions,
                $($extra_params)*
            )
            {
                // Things all wasm types have
                ty: naga::Handle<naga::Type>,
                default: |ty| naga::Handle<naga::Constant>,

                size_bytes: u32,
                make_const: |ty| MakeConstFn<$wasm_ty>,

                read_input: |ty| naga::Handle<naga::Function>,
                write_output: |ty| naga::Handle<naga::Function>,
                read_memory: |ty| naga::Handle<naga::Function>,
                write_memory: |ty| naga::Handle<naga::Function>,

                $($impl)*
            } with pub(crate) trait $trait_name;
        }
    };
    // The implementation required for numerics (i32, i64, f32, f64)
    // See https://webassembly.github.io/spec/core/syntax/instructions.html#numeric-instructions
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [numeric $(, $parts:tt)*]; {$($impl:tt)*}; ($($extra_params:tt)*)) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts),*]; {
            $($impl)*

            load:  |ty, read_memory|  naga::Handle<naga::Function>,
            store: |ty, write_memory| naga::Handle<naga::Function>,

            add: |ty| naga::Handle<naga::Function>,
            sub: |ty| naga::Handle<naga::Function>,
            mul: |ty| naga::Handle<naga::Function>,

            eq: |ty| naga::Handle<naga::Function>,
            ne: |ty| naga::Handle<naga::Function>,
        }; ($($extra_params)*)}
    };
    // The implementation required for integers (i32, i64)
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [integer $(, $parts:tt)*]; {$($impl:tt)*}; ($($extra_params:tt)*)) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts),*]; {
            $($impl)*

            // Memory
            load_8_u:  |ty, default, read_memory|  naga::Handle<naga::Function>,
            load_8_s:  |ty, default, read_memory|  naga::Handle<naga::Function>,
            load_16_u: |ty, default, read_memory|  naga::Handle<naga::Function>,
            load_16_s: |ty, default, read_memory|  naga::Handle<naga::Function>,
            store_8:   |ty, default, write_memory| naga::Handle<naga::Function>,
            store_16:  |ty, default, write_memory| naga::Handle<naga::Function>,

            // Comparisons
            eqz: |ty| naga::Handle<naga::Function>,

            lt_s: |ty| naga::Handle<naga::Function>,
            le_s: |ty| naga::Handle<naga::Function>,
            gt_s: |ty| naga::Handle<naga::Function>,
            ge_s: |ty| naga::Handle<naga::Function>,

            lt_u: |ty| naga::Handle<naga::Function>,
            le_u: |ty| naga::Handle<naga::Function>,
            gt_u: |ty| naga::Handle<naga::Function>,
            ge_u: |ty| naga::Handle<naga::Function>,

            // Operations
            clz: |ty| naga::Handle<naga::Function>,
            ctz: |ty| naga::Handle<naga::Function>,
            div_s: |ty| naga::Handle<naga::Function>,
            div_u: |ty| naga::Handle<naga::Function>,
            rem_s: |ty| naga::Handle<naga::Function>,
            rem_u: |ty| naga::Handle<naga::Function>,
            rotl: |ty| naga::Handle<naga::Function>,
            rotr: |ty| naga::Handle<naga::Function>,
            popcnt: |ty| naga::Handle<naga::Function>,
            and: |ty| naga::Handle<naga::Function>,
            or: |ty| naga::Handle<naga::Function>,
            xor: |ty| naga::Handle<naga::Function>,
            shl: |ty| naga::Handle<naga::Function>,
            shr_s: |ty| naga::Handle<naga::Function>,
            shr_u: |ty| naga::Handle<naga::Function>,

            // Extensions
            extend_8_s: |ty| naga::Handle<naga::Function>,
            extend_16_s: |ty| naga::Handle<naga::Function>,

            // Atomics (from thread proposal)
            /*atomic_load:             |ty, default, word| naga::Handle<naga::Function>,
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
            atomic_rmw_16_cmpxchg_u: |ty, default, word| naga::Handle<naga::Function>,*/
        }; ($($extra_params)*)}
    };
    // Just i64
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [i64 $(, $parts:tt)*]; {$($impl:tt)*}; ($($extra_params:tt)*)) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts),*]; {
            $($impl)*

            load_32_u:  |ty, default, read_memory|  naga::Handle<naga::Function>,
            load_32_s:  |ty, default, read_memory|  naga::Handle<naga::Function>,
            store_32:   |ty, default, write_memory| naga::Handle<naga::Function>,

            // Extensions
            extend_32_s: |ty| naga::Handle<naga::Function>,

            // Atomics (from thread proposal)
            /*atomic_load_32_u:        |ty, default, word| naga::Handle<naga::Function>,
            atomic_store_32:         |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_add_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_sub_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_and_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_or_u:      |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_xor_u:     |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_xchg_u:    |ty, default, word| naga::Handle<naga::Function>,
            atomic_rmw_32_cmpxchg_u: |ty, default, word| naga::Handle<naga::Function>,*/
        }; ($($extra_params)*)}
    };
    // The implementation required for floats (f32, f64)
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [floating $(, $parts:tt)*]; {$($impl:tt)*}; ($($extra_params:tt)*)) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts),*]; {
            $($impl)*

            abs: |ty| naga::Handle<naga::Function>,
            neg: |ty| naga::Handle<naga::Function>,
            ceil: |ty| naga::Handle<naga::Function>,
            floor: |ty| naga::Handle<naga::Function>,
            trunc: |ty| naga::Handle<naga::Function>,
            nearest: |ty| naga::Handle<naga::Function>,

            div: |ty| naga::Handle<naga::Function>,
            sqrt: |ty| naga::Handle<naga::Function>,
            min: |ty| naga::Handle<naga::Function>,
            max: |ty| naga::Handle<naga::Function>,
            copy_sign: |ty| naga::Handle<naga::Function>,

            lt: |ty| naga::Handle<naga::Function>,
            le: |ty| naga::Handle<naga::Function>,
            gt: |ty| naga::Handle<naga::Function>,
            ge: |ty| naga::Handle<naga::Function>,
        }; ($($extra_params)*)}
    };
    // Just f32
    (struct $struct_name:ident; trait $trait_name:ident; $wasm_ty:ty; [f32 $(, $parts:tt)*]; {$($impl:tt)*}; ($($extra_params:tt)*)) => {
        wasm_ty_generator!{struct $struct_name; trait $trait_name; $wasm_ty; [$($parts),*]; {
            $($impl)*

            convert_i32_s: |ty| naga::Handle<naga::Function>,
            convert_i32_u: |ty| naga::Handle<naga::Function>,
        }; ($($extra_params)* i32_ty: naga::Handle<naga::Type>,)}
    };
}

wasm_ty_generator!(struct I32Instance; trait I32Gen; i32; [numeric, integer]);
wasm_ty_generator!(struct I64Instance; trait I64Gen; i64; [numeric, integer, i64]);
wasm_ty_generator!(struct F32Instance; trait F32Gen; f32; [numeric, floating, f32]);
wasm_ty_generator!(struct F64Instance; trait F64Gen; f64; [numeric, floating]);
wasm_ty_generator!(struct V128Instance; trait V128Gen; V128; []);
wasm_ty_generator!(struct FuncRefInstance; trait FuncRefGen; FuncRef; []);
wasm_ty_generator!(struct ExternRefInstance; trait ExternRefGen; ExternRef; []);

fn make_64_bit_const_expr_from_2vec32(
    ty: naga::Handle<naga::Type>,
    const_expressions: &mut naga::Arena<naga::Expression>,
    value: i64,
) -> naga::Handle<naga::Expression> {
    let bytes = value.to_le_bytes();
    let components = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| {
            let word = u32::from_le_bytes(*bytes);
            let word = u32::try_from(word & 0xFFFFFFFF).expect("truncated word always fits in u32");
            const_expressions.append_u32(word)
        })
        .collect();
    let expr = const_expressions.append(
        naga::Expression::Compose { ty, components },
        naga::Span::UNDEFINED,
    );
    return expr;
}

/// Something of the form `f(A, A) -> Bool` which can be implemented with native functions
macro_rules! impl_native_bool_binexp {
    ($instance_gen:ident, $name:ident, $op_name:ident; $op:tt) => {
        paste::paste! {
            fn [< gen_ $op_name >](
                module: &mut naga::Module,
                requirements: $instance_gen::[< $op_name:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $op_name:camel >]> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _ $op_name >](lhs: *requirements.ty, rhs: *requirements.ty) -> requirements.preamble.wasm_bool.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let t = naga_expr!(&mut ctx => Constant(requirements.preamble.wasm_bool.const_true));
                let f = naga_expr!(&mut ctx => Constant(requirements.preamble.wasm_bool.const_false));
                let res = naga_expr!(&mut ctx => if (lhs $op rhs) {t} else {f});
                ctx.result(res);

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
                requirements: $instance_gen::[< $op_name:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $op_name:camel >]> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _ $op_name >](lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let res = naga_expr!(&mut ctx => lhs $op rhs);
                ctx.result(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_native_inner_binexp;

/// Something of the form `f(A, A) -> Bool` which can be implemented with native functions and bitcasts to unsigned
macro_rules! impl_native_unsigned_bool_binexp {
    ($instance_gen:ident, $name:ident, $op_name:ident; $op:tt) => {
        paste::paste! {
            fn [< gen_ $op_name >](
                module: &mut naga::Module,
                requirements: $instance_gen::[< $op_name:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $op_name:camel >]> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _ $op_name >](lhs: *requirements.ty, rhs: *requirements.ty) -> requirements.preamble.wasm_bool.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let t = naga_expr!(&mut ctx => Constant(requirements.preamble.wasm_bool.const_true));
                let f = naga_expr!(&mut ctx => Constant(requirements.preamble.wasm_bool.const_false));
                let res = naga_expr!(&mut ctx => if ((lhs as Uint) $op (rhs as Uint)) {t} else {f});
                ctx.result(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_native_unsigned_bool_binexp;

/// Something of the form `f(A, A) -> A` which can be implemented with native functions and bitcasts to and from unsigned
macro_rules! impl_native_unsigned_inner_binexp {
    ($instance_gen:ident, $name:ident, $op_name:ident; $op:tt) => {
        paste::paste! {
            fn [< gen_ $op_name >](
                module: &mut naga::Module,
                requirements: $instance_gen::[< $op_name:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $op_name:camel >]> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _ $op_name >](lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let res = naga_expr!(&mut ctx => ((lhs as Uint) $op (rhs as Uint)) as Sint);
                ctx.result(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_native_unsigned_inner_binexp;

/// Something of the form `f(A) -> A` which can be implemented with an inbuilt math function
macro_rules! impl_native_unary_inner_math_fn {
    ($instance_gen:ident, $name:ident, $op_name:ident; $op:ident) => {
        paste::paste! {
            fn [< gen_ $op_name >](
                module: &mut naga::Module,
                requirements: $instance_gen::[< $op_name:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $op_name:camel >]> {
                let (function_handle, value) = declare_function! {
                    module => fn [< $name _ $op_name >](value: *requirements.ty) -> *requirements.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let res = ctx.append_expr(naga::Expression::Math {
                    fun: naga::MathFunction::$op,
                    arg: value,
                    arg1: None,
                    arg2: None,
                    arg3: None
                });
                ctx.result(res);

                Ok(function_handle)
            }
        }
    };
}
use impl_native_unary_inner_math_fn;

/// A placeholder for something of the form `f(A, A) -> A`
macro_rules! impl_dud_inner_binexp {
    ($instance_gen:ident, $name:ident, $op_name:ident) => {
        paste::paste! {
            fn [< gen_ $op_name >](
                module: &mut naga::Module,
                requirements: $instance_gen::[< $op_name:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $op_name:camel >]> {
                let (function_handle, lhs, _) = declare_function! {
                    module => fn [< $name _ $op_name >](lhs: *requirements.ty, rhs: *requirements.ty) -> *requirements.ty
                };
                let ctx = BlockContext::from((module, function_handle));
                ctx.result(lhs);

                Ok(function_handle)
            }
        }
    };
}
use impl_dud_inner_binexp;

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
                requirements: $instance_gen::EqRequirements,
            ) -> build::Result<$instance_gen::Eq> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _eq >](lhs: *requirements.ty, rhs: *requirements.ty) -> requirements.preamble.wasm_bool.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let t = naga_expr!(&mut ctx => Constant(requirements.preamble.wasm_bool.const_true));
                let f = naga_expr!(&mut ctx => Constant(requirements.preamble.wasm_bool.const_false));

                let lhs_high = naga_expr!(&mut ctx => lhs[const 0]);
                let lhs_low = naga_expr!(&mut ctx => lhs[const 1]);
                let rhs_high = naga_expr!(&mut ctx => rhs[const 0]);
                let rhs_low = naga_expr!(&mut ctx => rhs[const 1]);
                let res = naga_expr!(&mut ctx => if ((lhs_low == rhs_low) & (lhs_high == rhs_high)) {t} else {f});
                ctx.result(res);

                Ok(function_handle)
            }

            fn gen_ne(
                module: &mut naga::Module,
                requirements: $instance_gen::NeRequirements,
            ) -> build::Result<$instance_gen::Ne> {
                let (function_handle, lhs, rhs) = declare_function! {
                    module => fn [< $name _ne >](lhs: *requirements.ty, rhs: *requirements.ty) -> requirements.preamble.wasm_bool.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let t = naga_expr!(&mut ctx => Constant(requirements.preamble.wasm_bool.const_true));
                let f = naga_expr!(&mut ctx => Constant(requirements.preamble.wasm_bool.const_false));

                let lhs_high = naga_expr!(&mut ctx => lhs[const 0]);
                let lhs_low = naga_expr!(&mut ctx => lhs[const 1]);
                let rhs_high = naga_expr!(&mut ctx => rhs[const 0]);
                let rhs_low = naga_expr!(&mut ctx => rhs[const 1]);
                let res = naga_expr!(&mut ctx => if ((lhs_low != rhs_low) | (lhs_high != rhs_high)) {t} else {f});
                ctx.result(res);

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
                requirements: $instance_gen::LoadRequirements,
            ) -> build::Result<$instance_gen::Load> {
                let (function_handle, memory, address) = declare_function! {
                    module => fn [< $name _load >](memory: requirements.preamble.word_ty, address: requirements.preamble.word_ty) -> *requirements.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                // TODO: Support other memories
                drop(memory);

                // Variable to unify aligned and unaligned loads
                let loaded_value_local = ctx.new_local(
                    "loaded_value".to_owned(),
                    *requirements.ty,
                    None,
                );
                let loaded_value_ptr = ctx.local_expr(loaded_value_local);

                // Test for unalignment
                let is_aligned = naga_expr!(&mut ctx => (address & U32(3)) == U32(0));
                ctx.test(is_aligned).then(|mut ctx| {
                    // Load aligned
                    let aligned_address = naga_expr!(&mut ctx => address >> U32(2));
                    let load_fn = *requirements.read_memory;
                    let load_result = ctx.call_get_return(load_fn, vec![aligned_address]);
                    ctx.store(loaded_value_ptr, load_result);
                }).otherwise(|mut ctx| {
                    // We can do things much faster if the address is word-aligned
                    // Otherwise we have to load twice and merge
                    // TODO: unaligned path
                    let aligned_address = naga_expr!(&mut ctx => address >> U32(2));
                    let load_fn = *requirements.read_memory;
                    let load_result = ctx.call_get_return(load_fn, vec![aligned_address]);
                    ctx.store(loaded_value_ptr, load_result);
                    // END TODO
                });

                // Then unify load paths
                let res = naga_expr!(&mut ctx => Load(loaded_value_ptr));
                ctx.result(res);

                Ok(function_handle)
            }

            fn gen_store(
                module: &mut naga::Module,
                requirements: $instance_gen::StoreRequirements,
            ) -> build::Result<$instance_gen::Store> {
                let (function_handle, memory, address, value) = declare_function! {
                    module => fn [< $name _store >](memory: requirements.preamble.word_ty, address: requirements.preamble.word_ty, value: *requirements.ty)
                };
                let mut ctx = BlockContext::from((module, function_handle));

                // TODO: Support other memories
                drop(memory);

                // If we have trapped, don't store
                let trap_state = requirements.preamble.trap_state;
                let is_trapped = naga_expr!(&mut ctx => Load(Global(trap_state)) != U32(0));
                ctx.test(is_trapped).then(|ctx| {
                    ctx.void_return();
                });

                // Test for unalignment
                let is_aligned = naga_expr!(&mut ctx => (address & U32(3)) == U32(0));
                ctx.test(is_aligned).then(|mut ctx| {
                    // Store aligned
                    let aligned_address = naga_expr!(&mut ctx => address >> U32(2));
                    let store_fn = *requirements.write_memory;
                    ctx.call_void(store_fn, vec![aligned_address, value]);
                }).otherwise(|mut ctx| {
                    // We can do things much faster if the address is word-aligned
                    // Otherwise we have to load twice, merge in value, and store back

                    // TODO: unaligned path
                    let aligned_address = naga_expr!(&mut ctx => address >> U32(2));
                    let store_fn = *requirements.write_memory;
                    ctx.call_void(
                        store_fn,
                        vec![aligned_address, value],
                    );
                    // END TODO
                });


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
                requirements: $instance_gen::[< $fn:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $fn:camel >]> {
                let (function_handle, ..) = declare_function! {
                    module => fn [< $name _ $fn >](memory: requirements.preamble.word_ty, address: requirements.preamble.word_ty) -> *requirements.ty
                };
                let mut ctx = BlockContext::from((module, function_handle));

                let default = naga_expr!(&mut ctx => Constant(*requirements.default));
                ctx.result(default);

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
                requirements: $instance_gen::[< $fn:camel Requirements >],
            ) -> build::Result<$instance_gen::[< $fn:camel >]> {
                let (function_handle, ..) = declare_function! {
                    module => fn [< $name _ $fn >](memory: requirements.preamble.word_ty, address: requirements.preamble.word_ty, value: *requirements.ty)
                };

                Ok(function_handle)
            }
        }
    };
}
use impl_dud_integer_store;

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
use naga_ext::ExpressionsExt;

/*
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
use impl_integer_atomic_loads_and_stores;*/
