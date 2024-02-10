use std::collections::HashMap;

use crate::typed::FuncRef;

use crate::{active_function::InactiveFunction, active_module::ActiveModule};

pub(crate) struct FunctionLookup<F> {
    lookup: HashMap<FuncRef, F>,
}

impl<F> FunctionLookup<F> {
    pub(crate) fn empty() -> Self {
        Self {
            lookup: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, ptr: FuncRef, handle: F) {
        self.lookup.insert(ptr, handle);
    }

    pub(crate) fn lookup(&self, ptr: &FuncRef) -> &F {
        self.lookup
            .get(ptr)
            .expect("all pointers are present from constructor")
    }
}

impl<F: InactiveFunction> FunctionLookup<F> {
    pub(crate) fn lookup_mut<'f>(
        &'f self,
        module: &'f mut ActiveModule<'f>,
        ptr: &FuncRef,
    ) -> F::Active<'f> {
        self.lookup(ptr).activate(module)
    }
}
