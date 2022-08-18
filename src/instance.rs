use crate::func::{Func, TypedFunc};
use crate::typed::WasmTyVec;
use crate::{Backend, Engine, Extern, Module};
use anyhow::Context;
use std::sync::Arc;

pub struct Instance<T>
where
    T: Backend,
{
    backend: Arc<T>,
}

impl<T> Instance<T>
where
    T: Backend,
{
    pub async fn new(
        engine: &Engine<T>,
        _module: &Module<T>,
        _imports: impl Into<&[Extern]>,
    ) -> anyhow::Result<Self> {
        return Ok(Self {
            backend: engine.backend(),
        });
    }

    /// Create an exported function that doesn't track its types, useful for runtime imports.
    /// Prefer instantiate_typed_function if possible, and see instantiate_typed_function for detail
    /// about this function.
    pub fn get_func(&self, _name: &str) -> anyhow::Result<Func> {
        todo!()
    }

    /// Create an exported function that tracks its types.
    /// This function communicates with the parallel computation device (e.g. GPU) so has a high overhead.
    /// Prefer calling once and reusing the returned exported function.
    pub fn get_typed_func<Params, Results>(
        &self,
        name: &str,
    ) -> anyhow::Result<TypedFunc<Params, Results>>
    where
        Params: WasmTyVec,
        Results: WasmTyVec,
    {
        let untyped = self
            .get_func(name)
            .context(format!("failed to find function export `{}`", name))?;
        let typed = TypedFunc::<Params, Results>::try_from(untyped).context(format!(
            "failed to convert function `{}` to given type",
            name
        ))?;

        return Ok(typed);
    }
}
