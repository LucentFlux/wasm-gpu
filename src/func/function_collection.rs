use std::collections::HashMap;

use crate::FuncRef;

pub struct FunctionCollection {
    lookup: HashMap<FuncRef, naga::Handle<naga::Function>>,
}

impl FunctionCollection {
    pub fn new(
        functions: &mut naga::Arena<naga::Function>,
        ptrs: impl IntoIterator<Item = FuncRef>,
    ) -> Self {
        let mut lookup = HashMap::new();

        for ptr in ptrs.into_iter() {
            let new_handle = functions.append(naga::Function::default(), naga::Span::UNDEFINED);
            lookup.insert(ptr, new_handle);
        }

        Self { lookup }
    }

    pub fn lookup(&self, ptr: &FuncRef) -> naga::Handle<naga::Function> {
        self.lookup
            .get(ptr)
            .expect("all pointers are present from constructor")
            .clone()
    }
}
