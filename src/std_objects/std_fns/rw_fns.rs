use crate::{
    assembled_module::{build, WorkingModule},
    func_gen::{building::make_fn_return, WorkingFunction},
    naga_expr, naga_fn_def,
};

use super::BufferFnGen;

// fn<buffer>(word_address: u32) -> i32
pub(crate) struct ReadI32Gen {}
impl BufferFnGen for ReadI32Gen {
    fn gen_for(
        working: &mut WorkingModule,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = working.std_objs.tys.address.get(working)?;
        let i32_ty = working.std_objs.tys.wasm_i32.get(working)?;

        let (mut working, handle) = working.make_function()?;

        let (word_address,) = naga_fn_def! {
            working => fn read_i32(word_address: address_ty) -> i32_ty
        };

        let output_ref = working.get_fn_mut().expressions.append(
            naga::Expression::GlobalVariable(buffer),
            naga::Span::UNDEFINED,
        );

        let read_word = naga_expr!(working => output_ref[word_address]);
        make_fn_return(&mut working, read_word);

        Ok(handle)
    }
}

// fn<buffer>(word_address: u32, value: i32)
pub(crate) struct WriteI32Gen {}
impl BufferFnGen for WriteI32Gen {
    fn gen_for(
        working: &mut WorkingModule,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = working.std_objs.tys.address.get(working)?;
        let i32_ty = working.std_objs.tys.wasm_i32.get(working)?;

        let (mut working, handle) = working.make_function()?;

        let (word_address, value) = naga_fn_def! {
            working => fn write_i32(word_address: address_ty, value: i32_ty)
        };

        let output_ref = working.get_fn_mut().expressions.append(
            naga::Expression::GlobalVariable(buffer),
            naga::Span::UNDEFINED,
        );

        let write_word_loc = naga_expr!(working => output_ref[word_address]);

        working.get_fn_mut().body.push(
            naga::Statement::Store {
                pointer: write_word_loc,
                value,
            },
            naga::Span::UNDEFINED,
        );

        Ok(handle)
    }
}
