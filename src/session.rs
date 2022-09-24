use crate::{Backend, FuncPtr, Store};
use futures::future::BoxFuture;
use itertools::Itertools;
use std::marker::PhantomData;
use std::sync::Arc;
use wasmtime::Val;

pub struct SessionProperties {
    pub warp_size: u32,
}

/// A session represents a collection of commands being executed on a backend.
/// Any code with control flow will inevitably become unsynchronised, however the performance
/// benefit of this library comes from SIMD, so a session aims to track the execution progress
/// of a collection of commands and schedule them in batches. This comes with some heuristics,
/// which can be adjusted through SessionProperties objects.
pub struct Session<'a, B, T>
where
    B: Backend,
{
    backend: Arc<B>,
    tasks: Vec<(usize, &'a mut Store<B, T>, FuncPtr<B, T>, Vec<Val>)>,
}

impl<'a, B, T> Session<'a, B, T>
where
    B: Backend,
{
    pub fn new(
        backend: Arc<B>,
        entry_funcs: Vec<(usize, &'a mut Store<B, T>, FuncPtr<B, T>, Vec<Val>)>,
    ) -> Self {
        Self {
            backend,
            tasks: entry_funcs,
        }
    }

    pub fn run(self) -> BoxFuture<'a, Vec<anyhow::Result<Vec<Val>>>> {
        unimplemented!()
    }
}
