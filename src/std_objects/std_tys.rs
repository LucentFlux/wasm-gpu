use crate::{build, TRAP_FLAG_INDEX};

use super::Generator;

/// A type that attaches itself to a module the first time it is requested
pub(crate) trait TyGen: Generator<Generated = naga::Handle<naga::Type>> {}
impl<G: Generator<Generated = naga::Handle<naga::Type>>> TyGen for G {}

#[derive(Default)]
pub(crate) struct UVec3Gen;
impl Generator for UVec3Gen {
    type Generated = naga::Handle<naga::Type>;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        _others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Tri,
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}

#[derive(Default)]
pub(crate) struct U32Gen;
impl Generator for U32Gen {
    type Generated = naga::Handle<naga::Type>;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        _others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}

#[derive(Default)]
pub(crate) struct WordArrayBufferGen;
impl Generator for WordArrayBufferGen {
    type Generated = naga::Handle<naga::Type>;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        let word_ty = others.u32.gen(module, others)?;

        let word_array_ty = module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Array {
                    base: word_ty,
                    size: naga::ArraySize::Dynamic,
                    stride: 4,
                },
            },
            naga::Span::UNDEFINED,
        );

        Ok(word_array_ty)
    }
}

#[derive(Default)]
pub(crate) struct FlagsBufferGen;
impl Generator for FlagsBufferGen {
    type Generated = naga::Handle<naga::Type>;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        let word_ty = others.u32.gen(module, others)?;

        let flag_members = vec![naga::StructMember {
            name: Some("trap_flag".to_owned()),
            ty: word_ty,
            binding: None,
            offset: TRAP_FLAG_INDEX * 4,
        }];
        let flags_ty = module.types.insert(
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
