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

// fn<buffer>(word_address: u32) -> i64
pub(crate) struct ReadI64Gen {}
impl BufferFnGen for ReadI64Gen {
    fn gen_for(
        working: &mut WorkingModule,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = working.std_objs.tys.address.get(working)?;
        let i64_ty = working.std_objs.tys.wasm_i64.get(working)?;

        let (mut working, handle) = working.make_function()?;

        let (word_address,) = naga_fn_def! {
            working => fn read_i64(word_address: address_ty) -> i64_ty
        };

        let output_ref = working.get_fn_mut().expressions.append(
            naga::Expression::GlobalVariable(buffer),
            naga::Span::UNDEFINED,
        );

        let read_word1 = naga_expr!(working => output_ref[word_address]);
        let read_word2 = naga_expr!(working => output_ref[word_address + (U32(1))]);
        let read_value = naga_expr!(working => i64_ty((read_word1 as Uint), (read_word2 as Uint)));
        make_fn_return(&mut working, read_value);

        Ok(handle)
    }
}

// fn<buffer>(word_address: u32, value: i64)
pub(crate) struct WriteI64Gen {}
impl BufferFnGen for WriteI64Gen {
    fn gen_for(
        working: &mut WorkingModule,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let address_ty = working.std_objs.tys.address.get(working)?;
        let i64_ty = working.std_objs.tys.wasm_i64.get(working)?;

        let (mut working, handle) = working.make_function()?;

        let (word_address, value) = naga_fn_def! {
            working => fn write_i64(word_address: address_ty, value: i64_ty)
        };

        let output_ref = working.get_fn_mut().expressions.append(
            naga::Expression::GlobalVariable(buffer),
            naga::Span::UNDEFINED,
        );

        let write_word_loc1 = naga_expr!(working => output_ref[word_address]);
        let word1 = naga_expr!(working => (value[const 0]) as Sint);
        let write_word_loc2 = naga_expr!(working => output_ref[word_address + (U32(1))]);
        let word2 = naga_expr!(working => (value[const 1]) as Sint);

        working.get_fn_mut().body.push(
            naga::Statement::Store {
                pointer: write_word_loc1,
                value: word1,
            },
            naga::Span::UNDEFINED,
        );
        working.get_fn_mut().body.push(
            naga::Statement::Store {
                pointer: write_word_loc2,
                value: word2,
            },
            naga::Span::UNDEFINED,
        );

        Ok(handle)
    }
}
