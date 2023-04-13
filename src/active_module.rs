use std::ops::Deref;
use std::ops::DerefMut;

use crate::active_function::EntryFunction;
use crate::active_function::InternalFunction;
use crate::brain_function::BrainFunction;
use crate::build;
use crate::FuncUnit;
use crate::Tuneables;
use crate::{std_objects::StdObjects, FuncsInstance};
use wasm_types::FuncRef;

/// A module that we're currently building. Essentially a wrapper around a &mut naga::Module, but
/// that knows what functions it will be asked to populated, and which is opinionated about the
/// kind of things we're doing.
pub(crate) struct ActiveModule<'a> {
    pub module: &'a mut naga::Module,
    pub std_objs: StdObjects,
    pub workgroup_size: u32,
}

impl<'a> ActiveModule<'a> {
    /// Collate all of the data required to build a module.
    pub(crate) fn new(module: &'a mut naga::Module, tuneables: &Tuneables) -> build::Result<Self> {
        // Generate bindings used for all standard wasm things like types and globals
        let std_objs = StdObjects::from_tuneables(module, tuneables)?;

        Ok(Self {
            module,
            std_objs,
            workgroup_size: tuneables.workgroup_size,
        })
    }

    /// Forward declare a base function
    pub(crate) fn declare_base_function(
        &mut self,
        ptr: FuncRef,
        function_data: &FuncUnit,
    ) -> InternalFunction {
        InternalFunction::append_declaration_to(
            &mut self.module,
            &self.std_objs,
            "_base_impl",
            ptr,
            function_data,
        )
    }

    /// Forward declare a shader entry function
    pub(crate) fn declare_entry_function(
        &mut self,
        ptr: FuncRef,
        function_data: &FuncUnit,
    ) -> EntryFunction {
        EntryFunction::append_declaration_to(
            &mut self.module,
            &self.std_objs,
            ptr,
            self.workgroup_size,
        )
    }

    /// Forward declare a stack function
    pub(crate) fn declare_stack_function(
        &mut self,
        ptr: FuncRef,
        function_data: &FuncUnit,
    ) -> InternalFunction {
        InternalFunction::append_declaration_to(
            &mut self.module,
            &self.std_objs,
            "_stack_impl",
            ptr,
            function_data,
        )
    }

    /// Forward declare a brain function
    pub(crate) fn declare_brain_function(&mut self) -> BrainFunction {
        BrainFunction::append_declaration_to(&mut self.module)
    }

    pub(crate) fn make_constant(
        &mut self,
        value: wasm_types::Val,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        self.std_objs.make_constant(&mut self.module, value)
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
