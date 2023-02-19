use std::sync::Arc;

use wasm_types::FuncRef;
use wasmparser::{FuncType, ValType};

use wasm_opcodes::OperatorByProposal;

use crate::references::{DataIndex, ElementIndex, GlobalIndex, MemoryIndex, TableIndex};

/// Data from the parsed module shared by all functions, e.g. function types
#[derive(Debug)]
pub struct FunctionModuleData {
    pub types: Vec<wasmparser::Type>,
}

/// All data for each function in the module, without imports
#[derive(Debug, Clone)]
pub struct FuncData {
    pub ty: FuncType,
    pub locals: Vec<(u32, ValType)>,
    pub operators: Vec<OperatorByProposal>,
    pub module_data: Arc<FunctionModuleData>,
}

#[derive(Debug)]
pub struct FuncAccessible {
    pub func_index_lookup: Vec<FuncRef>,
    pub global_index_lookup: Vec<GlobalIndex>,
    pub element_index_lookup: Vec<ElementIndex>,
    pub table_index_lookup: Vec<TableIndex>,
    pub data_index_lookup: Vec<DataIndex>,
    pub memory_index_lookup: Vec<MemoryIndex>,
}

impl FuncAccessible {
    pub fn empty() -> Self {
        Self {
            func_index_lookup: Vec::new(),
            global_index_lookup: Vec::new(),
            element_index_lookup: Vec::new(),
            table_index_lookup: Vec::new(),
            data_index_lookup: Vec::new(),
            memory_index_lookup: Vec::new(),
        }
    }
}

/// All data for each function in the module, including all module objects that the function can access
#[derive(Debug, Clone)]
pub struct FuncInstance {
    pub func_data: FuncData,
    /// Pointers into the instantiated module set of all accessible objects
    pub accessible: Arc<FuncAccessible>,
}

/// Something that can be called, either an instance to be converted to shader code,
/// or an injected custom function
#[derive(Debug, Clone)]
pub enum FuncUnit {
    LocalFunction(FuncInstance),
    //CustomFunction {},
}

#[derive(Debug)]
pub struct FuncsInstance {
    pub wasm_functions: Vec<FuncUnit>,
}

impl FuncsInstance {
    pub fn all_funcrefs(&self) -> Vec<FuncRef> {
        self.wasm_functions
            .iter()
            .enumerate()
            .map(|(ptr, _)| {
                FuncRef::try_from(
                    u32::try_from(ptr).expect("64-bit GPU word sizes are unsupported"),
                )
                .expect("cannot have more than u32::MAX - 1 functions")
            })
            .collect()
    }

    pub fn get(&self, ptr: FuncRef) -> Option<&FuncUnit> {
        self.wasm_functions
            .get(usize::try_from(ptr.as_u32()?).expect("16-bit CPU architectures are unsupported"))
    }
}
