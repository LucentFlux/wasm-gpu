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
pub mod func_gen;

/// All data for each function in the module, without imports
#[perfect_derive(Debug)]
pub struct FuncData {
    pub ty: FuncType,
    pub locals: Vec<(u32, ValType)>,
    pub operators: Vec<OperatorByProposal>,
}

#[perfect_derive(Debug)]
pub struct FuncAccessible<T> {
    pub func_index_lookup: Vec<UntypedFuncPtr<T>>,
    pub global_index_lookup: Vec<AbstractGlobalPtr>,
    pub element_index_lookup: Vec<ElementPtr>,
    pub table_index_lookup: Vec<AbstractTablePtr>,
    pub data_index_lookup: Vec<DataPtr>,
    pub memory_index_lookup: Vec<AbstractMemoryPtr>,
}

/// All data for each function in the module, including all module objects that the function can access
#[perfect_derive(Debug)]
pub struct FuncInstance<T> {
    pub func_data: FuncData,
    pub accessible: Option<Arc<FuncAccessible<T>>>,
}
