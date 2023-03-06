use wasm_gpu_funcgen::FuncAccessible;

use crate::instance::data::DataPtr;
use crate::instance::element::ElementPtr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use crate::instance::table::builder::AbstractTablePtr;
use crate::{instance::global::builder::AbstractGlobalPtr, UntypedFuncPtr};

#[derive(Debug)]
pub struct FuncAccessiblePtrs {
    pub func_index_lookup: Vec<UntypedFuncPtr>,
    pub global_index_lookup: Vec<AbstractGlobalPtr>,
    pub element_index_lookup: Vec<ElementPtr>,
    pub table_index_lookup: Vec<AbstractTablePtr>,
    pub data_index_lookup: Vec<DataPtr>,
    pub memory_index_lookup: Vec<AbstractMemoryPtr>,
}
impl FuncAccessiblePtrs {
    pub(crate) fn to_indices(&self) -> FuncAccessible {
        FuncAccessible {
            func_index_lookup: self
                .func_index_lookup
                .iter()
                .map(|ptr| ptr.to_func_ref())
                .collect(),
            global_index_lookup: self
                .global_index_lookup
                .iter()
                .map(|ptr| ptr.to_index())
                .collect(),
            element_index_lookup: self
                .element_index_lookup
                .iter()
                .map(|ptr| ptr.to_index())
                .collect(),
            table_index_lookup: self
                .table_index_lookup
                .iter()
                .map(|ptr| ptr.to_index())
                .collect(),
            data_index_lookup: self
                .data_index_lookup
                .iter()
                .map(|ptr| ptr.to_index())
                .collect(),
            memory_index_lookup: self
                .memory_index_lookup
                .iter()
                .map(|ptr| ptr.to_index())
                .collect(),
        }
    }
}
