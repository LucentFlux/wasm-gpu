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
pub struct Session<'a> {
    stores: &'a mut DeviceStoreSet,
    tasks: Vec<(UntypedFuncPtr, Vec<Val>)>,
}

impl<'a> Session<'a> {
    pub fn new(
        stores: &'a mut DeviceStoreSet,
        entry_func: UntypedFuncPtr, // We want to enter at the same point
        args: Vec<Vec<Val>>,
    ) -> Self {
        let tasks = args.into_iter().map(|s| (entry_func.clone(), s)).collect();
        Self { stores, tasks }
    }

    pub fn run(self) -> BoxFuture<'a, Vec<anyhow::Result<Vec<Val>>>> {
        unimplemented!()
    }
}
