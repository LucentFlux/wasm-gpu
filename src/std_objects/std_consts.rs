use super::Generator;

/// A constant that attaches itself to a module the first time it is requested
pub(crate) trait ConstGen: Generator<Generated = naga::Handle<naga::Constant>> {}
impl<G: Generator<Generated = naga::Handle<naga::Constant>>> ConstGen for G {}
