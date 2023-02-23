use crate::{
    assembled_module::{build, WorkingModule},
    TRAP_FLAG_INDEX,
};

use super::TyGen;

pub(crate) struct I32ArrayBufferGen {}
impl TyGen for I32ArrayBufferGen {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let i32_ty = working.std_objs.tys.wasm_i32.get(working)?;

        let word_array_ty = working.module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Array {
                    base: i32_ty,
                    size: naga::ArraySize::Dynamic,
                    stride: 4,
                },
            },
            naga::Span::UNDEFINED,
        );

        Ok(word_array_ty)
    }
}

pub(crate) struct FlagsBufferGen {}
impl TyGen for FlagsBufferGen {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let i32_ty = working.std_objs.tys.wasm_i32.get(working)?;

        let flag_members = vec![naga::StructMember {
            name: Some("trap_flag".to_owned()),
            ty: i32_ty,
            binding: None,
            offset: TRAP_FLAG_INDEX * 4,
        }];
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
