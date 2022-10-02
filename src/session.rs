use crate::instance::func::AbstractUntypedFuncPtr;
use crate::typed::Val;
use crate::{Backend, StoreSet};
use futures::future::BoxFuture;
use itertools::Itertools;
use std::sync::Arc;

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
    stores: &'a mut StoreSet<B, T>,
    tasks: Vec<(FuncPtr<B, T>, Vec<Val>)>,
}

impl<'a, B, T> Session<'a, B, T>
where
    B: Backend,
{
    pub fn new(
        backend: Arc<B>,
        stores: &'a mut StoreSet<B, T>,
        entry_func: AbstractUntypedFuncPtr<B, T>, // We want to enter at the same point
        args: Vec<Vec<Val>>,
    ) -> Self {
        let tasks = stores.concrete(entry_func).zip_eq(args).collect_vec();
        Self {
            backend,
            stores,
            tasks,
        }
    }

    pub fn run(self) -> BoxFuture<'a, Vec<anyhow::Result<Vec<Val>>>> {
        unimplemented!()
    }
}
