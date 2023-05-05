use super::ActiveBlock;
use crate::build;
use wasm_opcodes::MVPOperator;
use wasm_types::Val;
use wasmtime_environ::Trap;

macro_rules! mono {
    ($state:ident, $ty:ident::$fn:ident) => {
        $state.pop_one_push_call_mono($state.std_objects().$ty.$fn)
    };
}

macro_rules! bi {
    ($state:ident, $ty:ident::$fn:ident) => {
        $state.pop_two_push_call_bi($state.std_objects().$ty.$fn)
    };
}

macro_rules! mem_load {
    ($state:ident, $memarg:ident, $ty:ident::$fn:ident) => {
        $state.pop_one_push_call_mem_func($memarg, $state.std_objects().$ty.$fn)
    };
}

macro_rules! mem_store {
    ($state:ident, $memarg:ident, $ty:ident::$fn:ident) => {
        $state.pop_two_call_mem_func($memarg, $state.std_objects().$ty.$fn)
    };
}

pub(super) fn eat_mvp_operator(
    state: &mut ActiveBlock<'_, '_>,
    operator: MVPOperator,
) -> build::Result<()> {
    match operator {
        MVPOperator::Nop => {
            /* Pass */
            Ok(())
        }
        MVPOperator::I32Const { value } => state.push_const_val(Val::I32(value)),
        MVPOperator::I64Const { value } => state.push_const_val(Val::I64(value)),
        MVPOperator::F32Const { value } => {
            state.push_const_val(Val::F32(f32::from_bits(value.bits())))
        }
        MVPOperator::F64Const { value } => {
            state.push_const_val(Val::F64(f64::from_bits(value.bits())))
        }
        MVPOperator::Unreachable => state.append_trap(Trap::UnreachableCodeReached),
        MVPOperator::Drop => {
            /* Pass for now */
            Ok(())
        }
        MVPOperator::LocalGet { local_index } => {
            let local_ptr = state.local_ptr(local_index);
            state.push(naga::Expression::Load { pointer: local_ptr });
            Ok(())
        }
        MVPOperator::LocalSet { local_index } => {
            let local_ptr = state.local_ptr(local_index);
            let value = state.pop();
            state.append(naga::Statement::Store {
                pointer: local_ptr,
                value,
            });
            Ok(())
        }
        MVPOperator::LocalTee { local_index } => {
            let local_ptr = state.local_ptr(local_index);
            let value = state.peek();
            state.append(naga::Statement::Store {
                pointer: local_ptr,
                value,
            });
            Ok(())
        }
        MVPOperator::Select => unimplemented!(),
        MVPOperator::GlobalGet { global_index } => unimplemented!(),
        MVPOperator::GlobalSet { global_index } => unimplemented!(),
        MVPOperator::I32Load { memarg } => mem_load!(state, memarg, i32::load),
        MVPOperator::I64Load { memarg } => mem_load!(state, memarg, i64::load),
        MVPOperator::F32Load { memarg } => mem_load!(state, memarg, f32::load),
        MVPOperator::F64Load { memarg } => mem_load!(state, memarg, f64::load),
        MVPOperator::I32Load8S { memarg } => mem_load!(state, memarg, i32::load_8_s),
        MVPOperator::I32Load8U { memarg } => mem_load!(state, memarg, i32::load_8_u),
        MVPOperator::I32Load16S { memarg } => mem_load!(state, memarg, i32::load_16_s),
        MVPOperator::I32Load16U { memarg } => mem_load!(state, memarg, i32::load_16_u),
        MVPOperator::I64Load8S { memarg } => mem_load!(state, memarg, i64::load_8_s),
        MVPOperator::I64Load8U { memarg } => mem_load!(state, memarg, i64::load_8_u),
        MVPOperator::I64Load16S { memarg } => mem_load!(state, memarg, i64::load_16_s),
        MVPOperator::I64Load16U { memarg } => mem_load!(state, memarg, i64::load_16_u),
        MVPOperator::I64Load32S { memarg } => mem_load!(state, memarg, i64::load_32_s),
        MVPOperator::I64Load32U { memarg } => mem_load!(state, memarg, i64::load_32_u),
        MVPOperator::I32Store { memarg } => mem_store!(state, memarg, i32::store),
        MVPOperator::I64Store { memarg } => mem_store!(state, memarg, i64::store),
        MVPOperator::F32Store { memarg } => mem_store!(state, memarg, f32::store),
        MVPOperator::F64Store { memarg } => mem_store!(state, memarg, f64::store),
        MVPOperator::I32Store8 { memarg } => mem_store!(state, memarg, i32::store_8),
        MVPOperator::I32Store16 { memarg } => mem_store!(state, memarg, i32::store_16),
        MVPOperator::I64Store8 { memarg } => mem_store!(state, memarg, i64::store_8),
        MVPOperator::I64Store16 { memarg } => mem_store!(state, memarg, i64::store_16),
        MVPOperator::I64Store32 { memarg } => mem_store!(state, memarg, i64::store_32),
        MVPOperator::MemorySize { mem, mem_byte } => unimplemented!(),
        MVPOperator::MemoryGrow { mem, mem_byte } => unimplemented!(),
        MVPOperator::I32Eqz => mono!(state, i32::eqz),
        MVPOperator::I32Eq => bi!(state, i32::eq),
        MVPOperator::I32Ne => bi!(state, i32::ne),
        MVPOperator::I32LtS => bi!(state, i32::lt_s),
        MVPOperator::I32LtU => bi!(state, i32::lt_u),
        MVPOperator::I32GtS => bi!(state, i32::gt_s),
        MVPOperator::I32GtU => bi!(state, i32::gt_u),
        MVPOperator::I32LeS => bi!(state, i32::le_s),
        MVPOperator::I32LeU => bi!(state, i32::le_u),
        MVPOperator::I32GeS => bi!(state, i32::ge_s),
        MVPOperator::I32GeU => bi!(state, i32::ge_u),
        MVPOperator::I64Eqz => mono!(state, i64::eqz),
        MVPOperator::I64Eq => bi!(state, i64::eq),
        MVPOperator::I64Ne => bi!(state, i64::ne),
        MVPOperator::I64LtS => bi!(state, i64::lt_s),
        MVPOperator::I64LtU => bi!(state, i64::lt_u),
        MVPOperator::I64GtS => bi!(state, i64::gt_s),
        MVPOperator::I64GtU => bi!(state, i64::gt_u),
        MVPOperator::I64LeS => bi!(state, i64::le_s),
        MVPOperator::I64LeU => bi!(state, i64::le_u),
        MVPOperator::I64GeS => bi!(state, i64::ge_s),
        MVPOperator::I64GeU => bi!(state, i64::ge_u),
        MVPOperator::F32Eq => bi!(state, f32::eq),
        MVPOperator::F32Ne => bi!(state, f32::ne),
        MVPOperator::F32Lt => unimplemented!(),
        MVPOperator::F32Gt => unimplemented!(),
        MVPOperator::F32Le => unimplemented!(),
        MVPOperator::F32Ge => unimplemented!(),
        MVPOperator::F64Eq => bi!(state, f64::eq),
        MVPOperator::F64Ne => bi!(state, f64::ne),
        MVPOperator::F64Lt => unimplemented!(),
        MVPOperator::F64Gt => unimplemented!(),
        MVPOperator::F64Le => unimplemented!(),
        MVPOperator::F64Ge => unimplemented!(),
        MVPOperator::I32Clz => bi!(state, i32::clz),
        MVPOperator::I32Ctz => bi!(state, i32::ctz),
        MVPOperator::I32Popcnt => bi!(state, i32::popcnt),
        MVPOperator::I32Add => bi!(state, i32::add),
        MVPOperator::I32Sub => bi!(state, i32::sub),
        MVPOperator::I32Mul => bi!(state, i32::mul),
        MVPOperator::I32DivS => bi!(state, i32::div_s),
        MVPOperator::I32DivU => bi!(state, i32::div_u),
        MVPOperator::I32RemS => bi!(state, i32::rem_s),
        MVPOperator::I32RemU => bi!(state, i32::rem_u),
        MVPOperator::I32And => bi!(state, i32::and),
        MVPOperator::I32Or => bi!(state, i32::or),
        MVPOperator::I32Xor => bi!(state, i32::xor),
        MVPOperator::I32Shl => bi!(state, i32::shl),
        MVPOperator::I32ShrS => bi!(state, i32::shr_s),
        MVPOperator::I32ShrU => bi!(state, i32::shr_u),
        MVPOperator::I32Rotl => bi!(state, i32::rotl),
        MVPOperator::I32Rotr => bi!(state, i32::rotr),
        MVPOperator::I64Clz => bi!(state, i64::clz),
        MVPOperator::I64Ctz => bi!(state, i64::ctz),
        MVPOperator::I64Popcnt => bi!(state, i64::popcnt),
        MVPOperator::I64Add => bi!(state, i64::add),
        MVPOperator::I64Sub => bi!(state, i64::sub),
        MVPOperator::I64Mul => bi!(state, i64::mul),
        MVPOperator::I64DivS => bi!(state, i64::div_s),
        MVPOperator::I64DivU => bi!(state, i64::div_u),
        MVPOperator::I64RemS => bi!(state, i64::rem_s),
        MVPOperator::I64RemU => bi!(state, i64::rem_u),
        MVPOperator::I64And => bi!(state, i64::and),
        MVPOperator::I64Or => bi!(state, i64::or),
        MVPOperator::I64Xor => bi!(state, i64::xor),
        MVPOperator::I64Shl => bi!(state, i64::shl),
        MVPOperator::I64ShrS => bi!(state, i64::shr_s),
        MVPOperator::I64ShrU => bi!(state, i64::shr_u),
        MVPOperator::I64Rotl => bi!(state, i64::rotl),
        MVPOperator::I64Rotr => bi!(state, i64::rotr),
        MVPOperator::F32Abs => unimplemented!(),
        MVPOperator::F32Neg => unimplemented!(),
        MVPOperator::F32Ceil => unimplemented!(),
        MVPOperator::F32Floor => unimplemented!(),
        MVPOperator::F32Trunc => unimplemented!(),
        MVPOperator::F32Nearest => unimplemented!(),
        MVPOperator::F32Sqrt => unimplemented!(),
        MVPOperator::F32Add => bi!(state, f32::add),
        MVPOperator::F32Sub => bi!(state, f32::sub),
        MVPOperator::F32Mul => bi!(state, f32::mul),
        MVPOperator::F32Div => bi!(state, f32::div),
        MVPOperator::F32Min => unimplemented!(),
        MVPOperator::F32Max => unimplemented!(),
        MVPOperator::F32Copysign => unimplemented!(),
        MVPOperator::F64Abs => unimplemented!(),
        MVPOperator::F64Neg => unimplemented!(),
        MVPOperator::F64Ceil => unimplemented!(),
        MVPOperator::F64Floor => unimplemented!(),
        MVPOperator::F64Trunc => unimplemented!(),
        MVPOperator::F64Nearest => unimplemented!(),
        MVPOperator::F64Sqrt => unimplemented!(),
        MVPOperator::F64Add => bi!(state, f64::add),
        MVPOperator::F64Sub => bi!(state, f64::sub),
        MVPOperator::F64Mul => bi!(state, f64::mul),
        MVPOperator::F64Div => bi!(state, f64::div),
        MVPOperator::F64Min => unimplemented!(),
        MVPOperator::F64Max => unimplemented!(),
        MVPOperator::F64Copysign => unimplemented!(),
        MVPOperator::I32WrapI64 => unimplemented!(),
        MVPOperator::I32TruncF32S => unimplemented!(),
        MVPOperator::I32TruncF32U => unimplemented!(),
        MVPOperator::I32TruncF64S => unimplemented!(),
        MVPOperator::I32TruncF64U => unimplemented!(),
        MVPOperator::I64ExtendI32S => unimplemented!(),
        MVPOperator::I64ExtendI32U => unimplemented!(),
        MVPOperator::I64TruncF32S => unimplemented!(),
        MVPOperator::I64TruncF32U => unimplemented!(),
        MVPOperator::I64TruncF64S => unimplemented!(),
        MVPOperator::I64TruncF64U => unimplemented!(),
        MVPOperator::F32ConvertI32S => unimplemented!(),
        MVPOperator::F32ConvertI32U => unimplemented!(),
        MVPOperator::F32ConvertI64S => unimplemented!(),
        MVPOperator::F32ConvertI64U => unimplemented!(),
        MVPOperator::F32DemoteF64 => unimplemented!(),
        MVPOperator::F64ConvertI32S => unimplemented!(),
        MVPOperator::F64ConvertI32U => unimplemented!(),
        MVPOperator::F64ConvertI64S => unimplemented!(),
        MVPOperator::F64ConvertI64U => unimplemented!(),
        MVPOperator::F64PromoteF32 => unimplemented!(),
        MVPOperator::I32ReinterpretF32 => unimplemented!(),
        MVPOperator::I64ReinterpretF64 => unimplemented!(),
        MVPOperator::F32ReinterpretI32 => unimplemented!(),
        MVPOperator::F64ReinterpretI64 => unimplemented!(),
    }
}
