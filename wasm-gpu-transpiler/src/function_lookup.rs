use std::collections::HashMap;

use wasm_types::FuncRef;

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
    pub(crate) fn lookup_mut<'f, 'm>(
        &'f self,
        module: &'f mut ActiveModule<'m>,
        ptr: &FuncRef,
    ) -> F::Active<'f, 'm> {
        self.lookup(ptr).activate(module)
    }
}
