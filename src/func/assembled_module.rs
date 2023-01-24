use crate::instance::func::FuncsInstance;

/// All of the functions and trampolines for a module, in wgpu objects ready to be called.
pub struct AssembledModule {}
impl AssembledModule {
    pub(crate) fn assemble<T>(functions: &FuncsInstance<T>) -> AssembledModule {
        Self {}
    }
}
