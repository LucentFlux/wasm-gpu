use super::{binary, mem_load, mem_store, unary, ActiveBlock};
use crate::{build, typed::Val};
use wasm_opcodes::proposals::MVPOperator;
use wasmtime_environ::Trap;

pub(super) fn eat_mvp_operator(
    state: &mut ActiveBlock<'_>,
    operator: &MVPOperator,
) -> build::Result<()> {
    match operator {
        MVPOperator::Nop => {
            /* Pass */
            Ok(())
        }
        MVPOperator::I32Const { value } => state.push_const_val(Val::I32(*value)),
        MVPOperator::I64Const { value } => state.push_const_val(Val::I64(*value)),
        MVPOperator::F32Const { value } => {
            state.push_const_val(Val::F32(f32::from_bits(value.bits())))
        }
        MVPOperator::F64Const { value } => {
            state.push_const_val(Val::F64(f64::from_bits(value.bits())))
        }
        MVPOperator::Unreachable => state.append_trap(Trap::UnreachableCodeReached),
        MVPOperator::Drop => {
            state.pop(); // And do nothing
            Ok(())
        }
        MVPOperator::LocalGet { local_index } => {
            let local_ptr = state.local_ptr(*local_index);
            state.push(naga::Expression::Load { pointer: local_ptr });
            Ok(())
        }
        MVPOperator::LocalSet { local_index } => {
            let local_ptr = state.local_ptr(*local_index);
            let value = state.pop();
            state.ctx.store(local_ptr, value);
            Ok(())
        }
        MVPOperator::LocalTee { local_index } => {
            let local_ptr = state.local_ptr(*local_index);
            let value = state.peek();
            state.ctx.store(local_ptr, value);
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
        MVPOperator::I32Eqz => unary!(state, i32::eqz),
        MVPOperator::I32Eq => binary!(state, i32::eq),
        MVPOperator::I32Ne => binary!(state, i32::ne),
        MVPOperator::I32LtS => binary!(state, i32::lt_s),
        MVPOperator::I32LtU => binary!(state, i32::lt_u),
        MVPOperator::I32GtS => binary!(state, i32::gt_s),
        MVPOperator::I32GtU => binary!(state, i32::gt_u),
        MVPOperator::I32LeS => binary!(state, i32::le_s),
        MVPOperator::I32LeU => binary!(state, i32::le_u),
        MVPOperator::I32GeS => binary!(state, i32::ge_s),
        MVPOperator::I32GeU => binary!(state, i32::ge_u),
        MVPOperator::I64Eqz => unary!(state, i64::eqz),
        MVPOperator::I64Eq => binary!(state, i64::eq),
        MVPOperator::I64Ne => binary!(state, i64::ne),
        MVPOperator::I64LtS => binary!(state, i64::lt_s),
        MVPOperator::I64LtU => binary!(state, i64::lt_u),
        MVPOperator::I64GtS => binary!(state, i64::gt_s),
        MVPOperator::I64GtU => binary!(state, i64::gt_u),
        MVPOperator::I64LeS => binary!(state, i64::le_s),
        MVPOperator::I64LeU => binary!(state, i64::le_u),
        MVPOperator::I64GeS => binary!(state, i64::ge_s),
        MVPOperator::I64GeU => binary!(state, i64::ge_u),
        MVPOperator::F32Eq => binary!(state, f32::eq),
        MVPOperator::F32Ne => binary!(state, f32::ne),
        MVPOperator::F32Lt => binary!(state, f32::lt),
        MVPOperator::F32Gt => binary!(state, f32::gt),
        MVPOperator::F32Le => binary!(state, f32::le),
        MVPOperator::F32Ge => binary!(state, f32::ge),
        MVPOperator::F64Eq => binary!(state, f64::eq),
        MVPOperator::F64Ne => binary!(state, f64::ne),
        MVPOperator::F64Lt => binary!(state, f64::lt),
        MVPOperator::F64Gt => binary!(state, f64::gt),
        MVPOperator::F64Le => binary!(state, f64::le),
        MVPOperator::F64Ge => binary!(state, f64::ge),
        MVPOperator::I32Clz => unary!(state, i32::clz),
        MVPOperator::I32Ctz => unary!(state, i32::ctz),
        MVPOperator::I32Popcnt => unary!(state, i32::popcnt),
        MVPOperator::I32Add => binary!(state, i32::add),
        MVPOperator::I32Sub => binary!(state, i32::sub),
        MVPOperator::I32Mul => binary!(state, i32::mul),
        MVPOperator::I32DivS => binary!(state, i32::div_s),
        MVPOperator::I32DivU => binary!(state, i32::div_u),
        MVPOperator::I32RemS => binary!(state, i32::rem_s),
        MVPOperator::I32RemU => binary!(state, i32::rem_u),
        MVPOperator::I32And => binary!(state, i32::and),
        MVPOperator::I32Or => binary!(state, i32::or),
        MVPOperator::I32Xor => binary!(state, i32::xor),
        MVPOperator::I32Shl => binary!(state, i32::shl),
        MVPOperator::I32ShrS => binary!(state, i32::shr_s),
        MVPOperator::I32ShrU => binary!(state, i32::shr_u),
        MVPOperator::I32Rotl => binary!(state, i32::rotl),
        MVPOperator::I32Rotr => binary!(state, i32::rotr),
        MVPOperator::I64Clz => unary!(state, i64::clz),
        MVPOperator::I64Ctz => unary!(state, i64::ctz),
        MVPOperator::I64Popcnt => unary!(state, i64::popcnt),
        MVPOperator::I64Add => binary!(state, i64::add),
        MVPOperator::I64Sub => binary!(state, i64::sub),
        MVPOperator::I64Mul => binary!(state, i64::mul),
        MVPOperator::I64DivS => binary!(state, i64::div_s),
        MVPOperator::I64DivU => binary!(state, i64::div_u),
        MVPOperator::I64RemS => binary!(state, i64::rem_s),
        MVPOperator::I64RemU => binary!(state, i64::rem_u),
        MVPOperator::I64And => binary!(state, i64::and),
        MVPOperator::I64Or => binary!(state, i64::or),
        MVPOperator::I64Xor => binary!(state, i64::xor),
        MVPOperator::I64Shl => binary!(state, i64::shl),
        MVPOperator::I64ShrS => binary!(state, i64::shr_s),
        MVPOperator::I64ShrU => binary!(state, i64::shr_u),
        MVPOperator::I64Rotl => binary!(state, i64::rotl),
        MVPOperator::I64Rotr => binary!(state, i64::rotr),
        MVPOperator::F32Abs => unary!(state, f32::abs),
        MVPOperator::F32Neg => unary!(state, f32::neg),
        MVPOperator::F32Ceil => unary!(state, f32::ceil),
        MVPOperator::F32Floor => unary!(state, f32::floor),
        MVPOperator::F32Trunc => unary!(state, f32::trunc),
        MVPOperator::F32Nearest => unary!(state, f32::nearest),
        MVPOperator::F32Sqrt => unary!(state, f32::sqrt),
        MVPOperator::F32Add => binary!(state, f32::add),
        MVPOperator::F32Sub => binary!(state, f32::sub),
        MVPOperator::F32Mul => binary!(state, f32::mul),
        MVPOperator::F32Div => binary!(state, f32::div),
        MVPOperator::F32Min => binary!(state, f32::min),
        MVPOperator::F32Max => binary!(state, f32::max),
        MVPOperator::F32Copysign => binary!(state, f32::copy_sign),
        MVPOperator::F64Abs => unary!(state, f64::abs),
        MVPOperator::F64Neg => unary!(state, f64::neg),
        MVPOperator::F64Ceil => unary!(state, f64::ceil),
        MVPOperator::F64Floor => unary!(state, f64::floor),
        MVPOperator::F64Trunc => unary!(state, f64::trunc),
        MVPOperator::F64Nearest => unary!(state, f64::nearest),
        MVPOperator::F64Sqrt => unary!(state, f64::sqrt),
        MVPOperator::F64Add => binary!(state, f64::add),
        MVPOperator::F64Sub => binary!(state, f64::sub),
        MVPOperator::F64Mul => binary!(state, f64::mul),
        MVPOperator::F64Div => binary!(state, f64::div),
        MVPOperator::F64Min => binary!(state, f64::min),
        MVPOperator::F64Max => binary!(state, f64::max),
        MVPOperator::F64Copysign => binary!(state, f64::copy_sign),
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
        MVPOperator::F32ConvertI32S => unary!(state, f32::convert_i32_s),
        MVPOperator::F32ConvertI32U => unary!(state, f32::convert_i32_u),
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
