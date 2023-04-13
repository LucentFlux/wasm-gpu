use std::{marker::PhantomData, sync::atomic::AtomicBool};

use once_cell::sync::OnceCell;
use perfect_derive::perfect_derive;

use crate::build;

use super::Generator;

/// A constant that attaches itself to a module the first time it is requested
pub(crate) trait ConstGen {
    fn gen<Ps: super::GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Constant>>;
}

#[perfect_derive(Default)]
pub(crate) struct LazyConst<I> {
    generating: AtomicBool,
    handle: OnceCell<build::Result<naga::Handle<naga::Constant>>>,
    _phantom: PhantomData<I>,
}

impl<I: ConstGen> Generator for LazyConst<I> {
    type Generated = naga::Handle<naga::Constant>;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
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
