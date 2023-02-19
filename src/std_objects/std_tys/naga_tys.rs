use crate::assembled_module::{build, WorkingModule};

use super::TyGen;

pub(crate) struct WorkgroupArgument {}
impl TyGen for WorkgroupArgument {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Tri,
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(working.module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
