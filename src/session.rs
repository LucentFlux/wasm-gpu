use crate::instance::func::UntypedFuncPtr;
use crate::typed::Val;
use crate::DeviceStoreSet;
use futures::future::BoxFuture;
use lib_hal::backend::Backend;
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
    stores: &'a mut DeviceStoreSet<B, T>,
    tasks: Vec<(UntypedFuncPtr<B, T>, Vec<Val>)>,
}

impl<'a, B, T> Session<'a, B, T>
where
    B: Backend,
{
    pub fn new(
        backend: Arc<B>,
        stores: &'a mut DeviceStoreSet<B, T>,
        entry_func: UntypedFuncPtr<B, T>, // We want to enter at the same point
        args: Vec<Vec<Val>>,
    ) -> Self {
        let tasks = args.into_iter().map(|s| (entry_func.clone(), s)).collect();
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
