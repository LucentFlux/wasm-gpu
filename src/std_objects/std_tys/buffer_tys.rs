use crate::{
    func::assembled_module::{build, WorkingModule},
    session::{INVOCATION_ID_FLAG_INDEX, TRAP_FLAG_INDEX},
};

use super::TyGen;

pub struct I32ArrayBuffer {}
impl TyGen for I32ArrayBuffer {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let i32_ty = working.std_objs.tys.wasm_i32.get(working)?;

        let word_array_ty = working.module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Array {
                    base: i32_ty,
                    size: naga::ArraySize::Dynamic,
                    stride: 1,
                },
            },
            naga::Span::UNDEFINED,
        );

        Ok(word_array_ty)
    }
}

pub struct FlagsBuffer {}
impl TyGen for FlagsBuffer {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let i32_ty = working.std_objs.tys.wasm_i32.get(working)?;

        let flag_members = vec![
            naga::StructMember {
                name: Some("trap_flag".to_owned()),
                ty: i32_ty,
                binding: None,
                offset: TRAP_FLAG_INDEX,
            },
            naga::StructMember {
                name: Some("invocation_id".to_owned()),
                ty: i32_ty,
                binding: None,
                offset: INVOCATION_ID_FLAG_INDEX,
            },
        ];
        let flags_ty = working.module.types.insert(
            naga::Type {
                name: Some("wasm_flags".to_owned()),
                inner: naga::TypeInner::Struct {
                    span: u32::try_from(flag_members.len() * 4).expect("static size"),
                    members: flag_members,
                },
            },
            naga::Span::UNDEFINED,
        );

        Ok(flags_ty)
    }
}
