use crate::instance::func::UntypedFuncPtr;
use crate::typed::Val;
use crate::DeviceStoreSet;
use futures::future::BoxFuture;

pub const MEMORY_BINDING_INDEX: u32 = 0;
pub const GLOBAL_BINDING_INDEX: u32 = 1;
pub const INPUT_BINDING_INDEX: u32 = 2;
pub const OUTPUT_BINDING_INDEX: u32 = 3;
pub const STACK_BINDING_INDEX: u32 = 4;
pub const TABLE_BINDING_INDEX: u32 = 5;
pub const DATA_BINDING_INDEX: u32 = 6;
pub const ELEMENT_BINDING_INDEX: u32 = 7;
pub const FLAGS_BINDING_INDEX: u32 = 8;

// Flags are 32-bits wide
pub const TRAP_FLAG_INDEX: u32 = 0;

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
