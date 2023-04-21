use super::{active_basic_block::ActiveBasicBlock, ActiveFunction};
use crate::build;
use wasm_opcodes::MVPOperator;
use wasm_types::Val;
use wasmtime_environ::Trap;

fn const_val<'f, 'm: 'f>(state: &mut ActiveBasicBlock<'_, 'f, 'm>, val: Val) -> build::Result<()> {
    let val = state.make_wasm_constant(val)?;
    state.push(naga::Expression::Constant(val));
    Ok(())
}

pub(super) fn eat_mvp_operator<'f, 'm: 'f>(
    state: &mut ActiveBasicBlock<'_, 'f, 'm>,
    operator: &MVPOperator,
) -> build::Result<()> {
    match operator {
        MVPOperator::Nop => {
            /* Pass */
            Ok(())
        }
        MVPOperator::I32Const { value } => const_val(state, Val::I32(*value)),
        MVPOperator::I64Const { value } => const_val(state, Val::I64(*value)),
        MVPOperator::F32Const { value } => const_val(state, Val::F32(f32::from_bits(value.bits()))),
        MVPOperator::F64Const { value } => const_val(state, Val::F64(f64::from_bits(value.bits()))),
        MVPOperator::Unreachable => state.append_trap(Trap::UnreachableCodeReached),
        MVPOperator::Drop => {
            /* Pass for now */
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
            state.append(naga::Statement::Store {
                pointer: local_ptr,
                value,
            });
            Ok(())
        }
        MVPOperator::LocalTee { local_index } => {
            let local_ptr = state.local_ptr(*local_index);
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
        MVPOperator::I32Load { memarg } => unimplemented!(),
        MVPOperator::I64Load { memarg } => unimplemented!(),
        MVPOperator::F32Load { memarg } => unimplemented!(),
        MVPOperator::F64Load { memarg } => unimplemented!(),
        MVPOperator::I32Load8S { memarg } => unimplemented!(),
        MVPOperator::I32Load8U { memarg } => unimplemented!(),
        MVPOperator::I32Load16S { memarg } => unimplemented!(),
        MVPOperator::I32Load16U { memarg } => unimplemented!(),
        MVPOperator::I64Load8S { memarg } => unimplemented!(),
        MVPOperator::I64Load8U { memarg } => unimplemented!(),
        MVPOperator::I64Load16S { memarg } => unimplemented!(),
        MVPOperator::I64Load16U { memarg } => unimplemented!(),
        MVPOperator::I64Load32S { memarg } => unimplemented!(),
        MVPOperator::I64Load32U { memarg } => unimplemented!(),
        MVPOperator::I32Store { memarg } => unimplemented!(),
        MVPOperator::I64Store { memarg } => unimplemented!(),
        MVPOperator::F32Store { memarg } => unimplemented!(),
        MVPOperator::F64Store { memarg } => unimplemented!(),
        MVPOperator::I32Store8 { memarg } => unimplemented!(),
        MVPOperator::I32Store16 { memarg } => unimplemented!(),
        MVPOperator::I64Store8 { memarg } => unimplemented!(),
        MVPOperator::I64Store16 { memarg } => unimplemented!(),
        MVPOperator::I64Store32 { memarg } => unimplemented!(),
        MVPOperator::MemorySize { mem, mem_byte } => unimplemented!(),
        MVPOperator::MemoryGrow { mem, mem_byte } => unimplemented!(),
        MVPOperator::I32Eqz => unimplemented!(),
        MVPOperator::I32Eq => unimplemented!(),
        MVPOperator::I32Ne => unimplemented!(),
        MVPOperator::I32LtS => unimplemented!(),
        MVPOperator::I32LtU => unimplemented!(),
        MVPOperator::I32GtS => unimplemented!(),
        MVPOperator::I32GtU => unimplemented!(),
        MVPOperator::I32LeS => unimplemented!(),
        MVPOperator::I32LeU => unimplemented!(),
        MVPOperator::I32GeS => unimplemented!(),
        MVPOperator::I32GeU => unimplemented!(),
        MVPOperator::I64Eqz => unimplemented!(),
        MVPOperator::I64Eq => unimplemented!(),
        MVPOperator::I64Ne => unimplemented!(),
        MVPOperator::I64LtS => unimplemented!(),
        MVPOperator::I64LtU => unimplemented!(),
        MVPOperator::I64GtS => unimplemented!(),
        MVPOperator::I64GtU => unimplemented!(),
        MVPOperator::I64LeS => unimplemented!(),
        MVPOperator::I64LeU => unimplemented!(),
        MVPOperator::I64GeS => unimplemented!(),
        MVPOperator::I64GeU => unimplemented!(),
        MVPOperator::F32Eq => unimplemented!(),
        MVPOperator::F32Ne => unimplemented!(),
        MVPOperator::F32Lt => unimplemented!(),
        MVPOperator::F32Gt => unimplemented!(),
        MVPOperator::F32Le => unimplemented!(),
        MVPOperator::F32Ge => unimplemented!(),
        MVPOperator::F64Eq => unimplemented!(),
        MVPOperator::F64Ne => unimplemented!(),
        MVPOperator::F64Lt => unimplemented!(),
        MVPOperator::F64Gt => unimplemented!(),
        MVPOperator::F64Le => unimplemented!(),
        MVPOperator::F64Ge => unimplemented!(),
        MVPOperator::I32Clz => unimplemented!(),
        MVPOperator::I32Ctz => unimplemented!(),
        MVPOperator::I32Popcnt => unimplemented!(),
        MVPOperator::I32Add => state.call_bi(state.std_objects().i32.add),
        MVPOperator::I32Sub => unimplemented!(),
        MVPOperator::I32Mul => unimplemented!(),
        MVPOperator::I32DivS => unimplemented!(),
        MVPOperator::I32DivU => unimplemented!(),
        MVPOperator::I32RemS => unimplemented!(),
        MVPOperator::I32RemU => unimplemented!(),
        MVPOperator::I32And => unimplemented!(),
        MVPOperator::I32Or => unimplemented!(),
        MVPOperator::I32Xor => unimplemented!(),
        MVPOperator::I32Shl => unimplemented!(),
        MVPOperator::I32ShrS => unimplemented!(),
        MVPOperator::I32ShrU => unimplemented!(),
        MVPOperator::I32Rotl => unimplemented!(),
        MVPOperator::I32Rotr => unimplemented!(),
        MVPOperator::I64Clz => unimplemented!(),
        MVPOperator::I64Ctz => unimplemented!(),
        MVPOperator::I64Popcnt => unimplemented!(),
        MVPOperator::I64Add => state.call_bi(state.std_objects().i64.add),
        MVPOperator::I64Sub => unimplemented!(),
        MVPOperator::I64Mul => unimplemented!(),
        MVPOperator::I64DivS => unimplemented!(),
        MVPOperator::I64DivU => unimplemented!(),
        MVPOperator::I64RemS => unimplemented!(),
        MVPOperator::I64RemU => unimplemented!(),
        MVPOperator::I64And => unimplemented!(),
        MVPOperator::I64Or => unimplemented!(),
        MVPOperator::I64Xor => unimplemented!(),
        MVPOperator::I64Shl => unimplemented!(),
        MVPOperator::I64ShrS => unimplemented!(),
        MVPOperator::I64ShrU => unimplemented!(),
        MVPOperator::I64Rotl => unimplemented!(),
        MVPOperator::I64Rotr => unimplemented!(),
        MVPOperator::F32Abs => unimplemented!(),
        MVPOperator::F32Neg => unimplemented!(),
        MVPOperator::F32Ceil => unimplemented!(),
        MVPOperator::F32Floor => unimplemented!(),
        MVPOperator::F32Trunc => unimplemented!(),
        MVPOperator::F32Nearest => unimplemented!(),
        MVPOperator::F32Sqrt => unimplemented!(),
        MVPOperator::F32Add => state.call_bi(state.std_objects().f32.add),
        MVPOperator::F32Sub => unimplemented!(),
        MVPOperator::F32Mul => unimplemented!(),
        MVPOperator::F32Div => unimplemented!(),
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
        MVPOperator::F64Add => state.call_bi(state.std_objects().f64.add),
        MVPOperator::F64Sub => unimplemented!(),
        MVPOperator::F64Mul => unimplemented!(),
        MVPOperator::F64Div => unimplemented!(),
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
