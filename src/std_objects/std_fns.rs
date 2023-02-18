use std::marker::PhantomData;

use once_cell::sync::OnceCell;

use crate::func::assembled_module::{build, WorkingModule};

mod rw_fns;

/// A function that attaches itself to a module the first time it is requested
pub trait FnGen {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Function>>;
}

pub struct LazyFn<I: FnGen> {
    handle: OnceCell<build::Result<naga::Handle<naga::Function>>>,
    _phantom: PhantomData<I>,
}

impl<I: FnGen> LazyFn<I> {
    pub fn new() -> Self {
        Self {
            handle: OnceCell::new(),
            _phantom: PhantomData,
        }
    }

    pub fn get(&self, working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Function>> {
        self.handle.get_or_init(|| I::gen(working)).clone()
    }
}

pub struct StdFnSet {
    pub read_i32: LazyFn<self::rw_fns::ReadI32>,
}

impl StdFnSet {
    pub fn new() -> Self {
        Self {
            read_i32: LazyFn::new(),
        }
    }
}
