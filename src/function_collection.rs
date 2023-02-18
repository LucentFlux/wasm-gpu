use std::collections::HashMap;

use crate::FuncRef;

use super::call_graph::CallOrder;

pub struct FunctionCollection {
    lookup: HashMap<FuncRef, naga::Handle<naga::Function>>,
}

impl FunctionCollection {
    pub fn new(functions: &mut naga::Arena<naga::Function>, call_order: &CallOrder) -> Self {
        let mut lookup = HashMap::new();

        for ptr in call_order.get_in_order() {
            let new_handle = functions.append(naga::Function::default(), naga::Span::UNDEFINED);
            lookup.insert(ptr.to_func_ref(), new_handle);
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
