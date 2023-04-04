use std::{marker::PhantomData, sync::atomic::AtomicBool};

use once_cell::sync::OnceCell;
use perfect_derive::perfect_derive;

use crate::assembled_module::build;

use super::{GenerationParameters, Generator};

/// A function that attaches itself to a module the first time it is requested
pub(crate) trait FnGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Function>>;
}

#[perfect_derive(Default)]
pub(crate) struct LazyFn<I> {
    generating: AtomicBool,
    handle: OnceCell<build::Result<naga::Handle<naga::Function>>>,
    _phantom: PhantomData<I>,
}

impl<I: FnGen> Generator for LazyFn<I> {
    type Generated = naga::Handle<naga::Function>;

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
                    panic!("loop detected in std objects when generating buffer function")
                }
                I::gen(module, others)
            })
            .clone()
    }
}

pub(crate) trait BufferFnGen {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
        buffer: naga::Handle<naga::GlobalVariable>,
    ) -> build::Result<naga::Handle<naga::Function>>;
}

pub(crate) trait BufferExtraction {
    fn get_buffer<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>>;
}

pub(crate) struct FromInputBuffer;
impl BufferExtraction for FromInputBuffer {
    fn get_buffer<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        others.bindings.input.gen(module, others)
    }
}

pub(crate) struct FromOutputBuffer;
impl BufferExtraction for FromOutputBuffer {
    fn get_buffer<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        others.bindings.output.gen(module, others)
    }
}

pub(crate) struct FromMemoryBuffer;
impl BufferExtraction for FromMemoryBuffer {
    fn get_buffer<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        others.bindings.memory.gen(module, others)
    }
}

#[perfect_derive(Default)]
pub(crate) struct LazyBufferFn<I, B>(LazyFn<Self>, PhantomData<(I, B)>);

impl<I: BufferFnGen, B: BufferExtraction> FnGen for LazyBufferFn<I, B> {
    fn gen<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::Function>> {
        let buffer = B::get_buffer(module, others)?;
        I::gen(module, others, buffer)
    }
}

impl<I: BufferFnGen, B: BufferExtraction> Generator for LazyBufferFn<I, B> {
    type Generated = <LazyFn<Self> as Generator>::Generated;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> crate::assembled_module::build::Result<Self::Generated> {
        self.0.gen(module, others)
    }
}
