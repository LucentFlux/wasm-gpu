use wasm_opcodes::ThreadsOperator;

use crate::{build, BuildError};

use super::ActiveBlock;

macro_rules! impl_op {
    (match $operator:ident {
        $state:ident {
        $(
            $state_function:ident {
                $($ty:ident :: $fn:ident),* $(,)*
            }
        ),* $(,)?
        }
        $($others:tt)*
    }) => {
        paste::paste!{
            match $operator {
                $($(
                    ThreadsOperator:: [< $ty:camel $fn:camel >] { memarg } => $state.$state_function(memarg, $state.std_objects().$ty.$fn),
                )*)*
                $($others)*
            }

        }
    };
}

pub(super) fn eat_threads_operator(
    state: &mut ActiveBlock<'_, '_>,
    operator: ThreadsOperator,
) -> build::Result<()> {
    let err = Err(BuildError::UnsupportedInstructionError {
        instruction_opcode: operator.opcode(),
    });
    return err;
    /*impl_op! {
        match operator {
            state {
                pop_one_push_call_mem_func {
                    i32::atomic_load,
                    i32::atomic_load_8_u,
                    i32::atomic_load_16_u,
                    i64::atomic_load,
                    i64::atomic_load_8_u,
                    i64::atomic_load_16_u,
                    i64::atomic_load_32_u,
                },

                pop_two_call_mem_func {
                    i32::atomic_store,
                    i32::atomic_store_8,
                    i32::atomic_store_16,
                    i64::atomic_store,
                    i64::atomic_store_8,
                    i64::atomic_store_16,
                    i64::atomic_store_32,

                    i32::atomic_rmw_add,
                    i32::atomic_rmw_8_add_u,
                    i32::atomic_rmw_16_add_u,
                    i64::atomic_rmw_add,
                    i64::atomic_rmw_8_add_u,
                    i64::atomic_rmw_16_add_u,
                    i64::atomic_rmw_32_add_u,

                    i32::atomic_rmw_sub,
                    i32::atomic_rmw_8_sub_u,
                    i32::atomic_rmw_16_sub_u,
                    i64::atomic_rmw_sub,
                    i64::atomic_rmw_8_sub_u,
                    i64::atomic_rmw_16_sub_u,
                    i64::atomic_rmw_32_sub_u,

                    i32::atomic_rmw_and,
                    i32::atomic_rmw_8_and_u,
                    i32::atomic_rmw_16_and_u,
                    i64::atomic_rmw_and,
                    i64::atomic_rmw_8_and_u,
                    i64::atomic_rmw_16_and_u,
                    i64::atomic_rmw_32_and_u,

                    i32::atomic_rmw_or,
                    i32::atomic_rmw_8_or_u,
                    i32::atomic_rmw_16_or_u,
                    i64::atomic_rmw_or,
                    i64::atomic_rmw_8_or_u,
                    i64::atomic_rmw_16_or_u,
                    i64::atomic_rmw_32_or_u,

                    i32::atomic_rmw_xor,
                    i32::atomic_rmw_8_xor_u,
                    i32::atomic_rmw_16_xor_u,
                    i64::atomic_rmw_xor,
                    i64::atomic_rmw_8_xor_u,
                    i64::atomic_rmw_16_xor_u,
                    i64::atomic_rmw_32_xor_u,

                    i32::atomic_rmw_xchg,
                    i32::atomic_rmw_8_xchg_u,
                    i32::atomic_rmw_16_xchg_u,
                    i64::atomic_rmw_xchg,
                    i64::atomic_rmw_8_xchg_u,
                    i64::atomic_rmw_16_xchg_u,
                    i64::atomic_rmw_32_xchg_u,

                    i32::atomic_rmw_cmpxchg,
                    i32::atomic_rmw_8_cmpxchg_u,
                    i32::atomic_rmw_16_cmpxchg_u,
                    i64::atomic_rmw_cmpxchg,
                    i64::atomic_rmw_8_cmpxchg_u,
                    i64::atomic_rmw_16_cmpxchg_u,
                    i64::atomic_rmw_32_cmpxchg_u,
                }
            }

            ThreadsOperator::MemoryAtomicNotify { memarg } => err,
            ThreadsOperator::MemoryAtomicWait32 { memarg } => err,
            ThreadsOperator::MemoryAtomicWait64 { memarg } => err,
            ThreadsOperator::AtomicFence => err,
        }
    }*/
}
