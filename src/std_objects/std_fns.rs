use std::marker::PhantomData;

use perfect_derive::perfect_derive;

use crate::build;

use super::{GenerationParameters, Generator};

/// A function that attaches itself to a module the first time it is requested
pub(super) trait FnGen: Generator<Generated = naga::Handle<naga::Function>> {}
impl<G: Generator<Generated = naga::Handle<naga::Function>>> FnGen for G {}

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

pub(crate) struct FromFlagsBuffer;
impl BufferExtraction for FromFlagsBuffer {
    fn get_buffer<Ps: GenerationParameters>(
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<naga::Handle<naga::GlobalVariable>> {
        others.bindings.flags.gen(module, others)
    }
}

#[perfect_derive(Default)]
pub(crate) struct BufferFn<G, B>(PhantomData<(G, B)>);

impl<G: BufferFnGen, B: BufferExtraction> Generator for BufferFn<G, B> {
    type Generated = naga::Handle<naga::Function>;

    fn gen<Ps: super::GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &super::StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        let buffer = B::get_buffer(module, others)?;
        G::gen(module, others, buffer)
    }
}
