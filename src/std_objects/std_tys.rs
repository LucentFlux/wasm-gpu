use std::{marker::PhantomData, sync::atomic::AtomicBool};

use once_cell::sync::OnceCell;
use perfect_derive::perfect_derive;

use crate::{
    assembled_module::{build, ActiveModule},
    TRAP_FLAG_INDEX,
};
use wasm_types::WasmTyVal;

use super::Generator;

/// A type that attaches itself to a module the first time it is requested
pub(crate) trait TyGen {
    fn gen<Ps: super::GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>>;
}

/// A type, linked to a wasm type, that links itself on first request
pub(crate) trait WasmTyGen: TyGen {
    type WasmTy: WasmTyVal;
    // Argument `ty` is passed in from a lazy evaluation of `Self::gen`
    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut ActiveModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>>;
}

#[perfect_derive(Default)]
pub(crate) struct LazyTy<I> {
    generating: AtomicBool,
    handle: OnceCell<build::Result<naga::Handle<naga::Type>>>,
    _phantom: PhantomData<I>,
}

impl<I: TyGen> Generator for LazyTy<I> {
    type Generated = naga::Handle<naga::Type>;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> crate::assembled_module::build::Result<Self::Generated> {
        self.handle
            .get_or_init(|| {
                if self
                    .generating
                    .fetch_or(true, std::sync::atomic::Ordering::AcqRel)
                {
                    panic!("loop detected in std objects when generating type")
                }
                I::gen(module, others)
                // No need to clear self.generating since we generate once
            })
            .clone()
    }
}

pub(crate) struct UVec3Gen;
impl TyGen for UVec3Gen {
    fn gen<Ps: super::GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
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

pub(crate) struct U32Gen;
impl TyGen for U32Gen {
    fn gen<Ps: super::GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
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

pub(crate) struct WordArrayBufferGen;
impl TyGen for WordArrayBufferGen {
    fn gen<Ps: super::GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
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

pub(crate) struct FlagsBufferGen {}
impl TyGen for FlagsBufferGen {
    fn gen<Ps: super::GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Type>> {
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
