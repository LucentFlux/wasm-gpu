use std::ops::Deref;
use std::ops::DerefMut;

use crate::active_function::EntryFunction;
use crate::active_function::InternalFunction;
use crate::brain_function::BrainFunction;
use crate::build;
use crate::std_objects::StdObjects;
use crate::FuncUnit;
use crate::Tuneables;
use wasm_types::FuncRef;

/// A module that we're currently building. Essentially a wrapper around a &mut naga::Module, but
/// that knows what functions it will be asked to populated, and which is opinionated about the
/// kind of things we're doing.
pub(crate) struct ActiveModule<'a> {
    pub module: &'a mut naga::Module,
    pub std_objects: StdObjects,
    pub workgroup_size: u32,
}

impl<'a> ActiveModule<'a> {
    /// Collate all of the data required to build a module.
    pub(crate) fn new(module: &'a mut naga::Module, tuneables: &Tuneables) -> build::Result<Self> {
        // Generate bindings used for all standard wasm things like types and globals
        let std_objects = StdObjects::from_tuneables(module, tuneables)?;

        Ok(Self {
            module,
            std_objects,
            workgroup_size: tuneables.workgroup_size,
        })
    }

    /// Forward declare a base function
    pub(crate) fn declare_base_function(
        &mut self,
        ptr: FuncRef,
        function_data: &FuncUnit,
    ) -> build::Result<InternalFunction> {
        InternalFunction::append_declaration_to(
            &mut self.module,
            &self.std_objects,
            "_base_impl",
            ptr,
            function_data,
        )
    }

    /// Forward declare a shader entry function
    pub(crate) fn declare_entry_function(&mut self, ptr: FuncRef) -> EntryFunction {
        EntryFunction::append_declaration_to(
            &mut self.module,
            &self.std_objects,
            ptr,
            self.workgroup_size,
        )
    }

    /// Forward declare a stack function
    pub(crate) fn declare_stack_function(
        &mut self,
        ptr: FuncRef,
        function_data: &FuncUnit,
    ) -> build::Result<InternalFunction> {
        InternalFunction::append_declaration_to(
            &mut self.module,
            &self.std_objects,
            "_stack_impl",
            ptr,
            function_data,
        )
    }

    /// Forward declare a brain function
    pub(crate) fn declare_brain_function(&mut self) -> BrainFunction {
        BrainFunction::append_declaration_to(&mut self.module)
    }
}

impl<'a> Deref for ActiveModule<'a> {
    type Target = naga::Module;

    fn deref(&self) -> &Self::Target {
        self.module
    }
}

impl<'a> DerefMut for ActiveModule<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.module
    }
}
