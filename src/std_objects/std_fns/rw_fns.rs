use crate::{
    func::{
        assembled_module::{build, WorkingModule},
        func_gen::{building::make_fn_return, WorkingFunction},
    },
    naga_expr, naga_fn_def,
};

use super::FnGen;

// fn(buffer, word_address: i32) -> i32
pub struct ReadI32 {}
impl FnGen for ReadI32 {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Function>> {
        let i32_ty = working.std_objs.tys.wasm_i32.get(working)?;
        let buffer_ty = working.std_objs.tys.wasm_i32_array_buffer.get(working)?;

        let (mut working, handle) = working.make_function();

        let (buffer, word_address) = naga_fn_def! {
            working => fn read_i32(buffer: buffer_ty, byte_address: i32_ty)
        };

        let read_word = naga_expr!(working => buffer[word_address]);
        make_fn_return(&mut working, read_word);

        Ok(handle)
    }
}
