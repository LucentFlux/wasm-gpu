use std::sync::Arc;

use perfect_derive::perfect_derive;
use wasmparser::{FuncType, ValType};

use crate::instance::data::DataPtr;
use crate::instance::element::ElementPtr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use crate::instance::table::builder::AbstractTablePtr;
use crate::{instance::global::builder::AbstractGlobalPtr, UntypedFuncPtr};

use crate::module::operation::OperatorByProposal;

pub mod assembled_module;
mod bindings_gen;
mod call_graph;
mod func_gen;
mod function_collection;

/// Data from the parsed module shared by all functions, e.g. function types
#[derive(Debug)]
pub struct FunctionModuleData {
    pub types: Vec<wasmparser::Type>,
}

/// All data for each function in the module, without imports
#[derive(Debug)]
pub struct FuncData {
    pub ty: FuncType,
    pub locals: Vec<(u32, ValType)>,
    pub operators: Vec<OperatorByProposal>,
    pub module_data: Arc<FunctionModuleData>,
}

#[derive(Debug)]
pub struct FuncAccessible {
    pub func_index_lookup: Vec<UntypedFuncPtr>,
    pub global_index_lookup: Vec<AbstractGlobalPtr>,
    pub element_index_lookup: Vec<ElementPtr>,
    pub table_index_lookup: Vec<AbstractTablePtr>,
    pub data_index_lookup: Vec<DataPtr>,
    pub memory_index_lookup: Vec<AbstractMemoryPtr>,
}

/// All data for each function in the module, including all module objects that the function can access
#[perfect_derive(Debug)]
pub struct FuncInstance {
    pub func_data: FuncData,
    /// Pointers into the instantiated module set of all accessible objects
    pub accessible: Option<Arc<FuncAccessible>>,
}

impl FuncInstance {
    pub fn accessible(&self) -> &FuncAccessible {
        self.accessible
            .as_ref()
            .expect("accessible values should be populated at module link time")
    }
}

/// Something that can be called, either an instance to be converted to shader code,
/// or an injected custom function
#[perfect_derive(Debug)]
pub enum FuncUnit {
    LocalFunction(FuncInstance),
    //CustomFunction {},
}
