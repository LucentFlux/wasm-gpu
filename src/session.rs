use crate::instance::func::UntypedFuncPtr;
use crate::typed::Val;
use crate::DeviceStoreSet;
use futures::future::BoxFuture;

pub struct SessionProperties {
    pub warp_size: u32,
}

/// A session represents a collection of commands being executed on a backend.
/// Any code with control flow will inevitably become unsynchronised, however the performance
/// benefit of this library comes from SIMD, so a session aims to track the execution progress
/// of a collection of commands and schedule them in batches. This comes with some heuristics,
/// which can be adjusted through SessionProperties objects.
pub struct Session<'a, T> {
    stores: &'a mut DeviceStoreSet<T>,
    tasks: Vec<(UntypedFuncPtr<T>, Vec<Val>)>,
}

impl<'a, T> Session<'a, T> {
    pub fn new(
        stores: &'a mut DeviceStoreSet<T>,
        entry_func: UntypedFuncPtr<T>, // We want to enter at the same point
        args: Vec<Vec<Val>>,
    ) -> Self {
        let tasks = args.into_iter().map(|s| (entry_func.clone(), s)).collect();
        Self { stores, tasks }
    }

    pub fn run(self) -> BoxFuture<'a, Vec<anyhow::Result<Vec<Val>>>> {
        unimplemented!()
    }
}
