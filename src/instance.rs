use crate::func::{Func, TypedFunc};
use crate::typed::WaspParams;
use crate::{Backend, Engine, Extern, Module};
use anyhow::Context;
use wasmtime::WasmResults;

pub struct Instance<T>
where
    T: Backend, {}

impl<T> Instance<T>
where
    T: Backend,
{
    pub async fn new(
        _engine: &Engine<T>,
        _module: &Module<T>,
        _imports: impl Into<&[Extern]>,
    ) -> anyhow::Result<Self> {
        todo!()
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
        Params: WaspParams,
        Results: WasmResults,
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
