use crate::func::{Func, TypedFunc};
use crate::{Backend, Engine, ExportedFunc, Extern};
use anyhow::{Context, Error};
use std::sync::Arc;
use wasmtime_environ::{ModuleEnvironment, ModuleTranslation};

/// A SPIR-V module that has not been told which parameters are Static and which are Ranges
pub struct Module<T>
where
    T: Backend,
{
    backend: Arc<T>,
}

impl<T> Module<T>
where
    T: Backend,
{
    pub fn new(engine: &Engine<T>, bytes: impl AsRef<[u8]>) -> Result<Self, Error> {
        let bytes = bytes.as_ref();
        let wasm = wat::parse_bytes(bytes)?;

        let mut validator =
            wasmparser::Validator::new_with_features(engine.config().features.clone());
        let parser = wasmparser::Parser::new(0);
        let mut types = Default::default();
        let _translation = ModuleEnvironment::new(tunables, &mut validator, &mut types)
            .translate(parser, &wasm)
            .context("failed to parse WebAssembly module")?;
        let _types = types.finish();

        todo!();

        return Ok(Self {
            backend: engine.backend(),
        });
    }
}
