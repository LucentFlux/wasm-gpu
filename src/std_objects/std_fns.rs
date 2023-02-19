use std::marker::PhantomData;

use once_cell::sync::OnceCell;

use crate::assembled_module::{build, WorkingModule};

mod rw_fns;

/// A function that attaches itself to a module the first time it is requested
pub(crate) trait FnGen {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Function>>;
}

pub(crate) struct LazyFn<I: FnGen> {
    handle: OnceCell<build::Result<naga::Handle<naga::Function>>>,
    _phantom: PhantomData<I>,
}

impl<I: FnGen> LazyFn<I> {
    pub(crate) fn new() -> Self {
        Self {
            handle: OnceCell::new(),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn get(
        &self,
        working: &mut WorkingModule,
    ) -> build::Result<naga::Handle<naga::Function>> {
        self.handle.get_or_init(|| I::gen(working)).clone()
    }
}

pub(crate) struct StdFnSet {
    pub(crate) read_i32: LazyFn<self::rw_fns::ReadI32>,
}

impl StdFnSet {
    pub(crate) fn new() -> Self {
        Self {
            read_i32: LazyFn::new(),
        }
    }
}
