use crate::Config;

/// An arena where everything is allocated during WASM building and execution.
/// When performing a set of operations to execute some WASM modules, the same engine
/// should be used for all related objects.
pub struct Engine {
    pub(crate) my_types: naga::UniqueArena<naga::Type>,
    pub(crate) config: Config,
}
impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            my_types: naga::UniqueArena::new(),
            config,
        }
    }
}
