use std::marker::PhantomData;

use once_cell::sync::OnceCell;

use crate::assembled_module::{build, WorkingModule};

mod rw_fns;

/// A function that attaches itself to a module the first time it is requested
pub(crate) trait FnGen {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Function>>;
}

pub(crate) struct LazyFn<I> {
    handle: OnceCell<build::Result<naga::Handle<naga::Function>>>,
    _phantom: PhantomData<I>,
}

impl<I> LazyFn<I> {
    pub(crate) fn new() -> Self {
        Self {
            handle: OnceCell::new(),
            _phantom: PhantomData,
        }
    }
}
impl<I: FnGen> LazyFn<I> {
    pub(crate) fn get(
        &self,
        working: &mut WorkingModule,
    ) -> build::Result<naga::Handle<naga::Function>> {
        self.handle.get_or_init(|| I::gen(working)).clone()
    }
}

pub(crate) trait BufferFnGen {
    fn gen_for(
        working: &mut WorkingModule,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>>;
}

pub(crate) struct LazyBufferFn<I> {
    handles: elsa::FrozenMap<
        naga::Handle<naga::GlobalVariable>,
        Box<build::Result<naga::Handle<naga::Function>>>,
    >,
    _phantom: PhantomData<I>,
}

impl<I> LazyBufferFn<I> {
    pub(crate) fn new() -> Self {
        Self {
            handles: elsa::FrozenMap::new(),
            _phantom: PhantomData,
        }
    }
}

impl<I: BufferFnGen> LazyBufferFn<I> {
    pub(crate) fn get_for(
        &self,
        working: &mut WorkingModule,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let mut res = self.handles.get(&buffer);
        match res {
            None => {
                self.handles
                    .insert(buffer, Box::new(I::gen_for(working, buffer)));
                self.get_for(working, buffer)
            }
            Some(res) => res.clone(),
        }
    }
}

pub(crate) struct StdFnSet {
    pub(crate) read_i32: LazyBufferFn<self::rw_fns::ReadI32Gen>,
    pub(crate) write_i32: LazyBufferFn<self::rw_fns::WriteI32Gen>,
}

impl StdFnSet {
    pub(crate) fn new() -> Self {
        Self {
            read_i32: LazyBufferFn::new(),
            write_i32: LazyBufferFn::new(),
        }
    }
}
