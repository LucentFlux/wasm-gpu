//! This module defines our interface to shader generation, i.e. how WASM should be specified when
//! handed off to this package.

use std::ops::Deref;
use std::sync::Arc;

use crate::typed::FuncRef;
use wasmparser::{FuncType, ValType};

use wasm_opcodes::OperatorByProposal;

macro_rules! impl_index {
    (pub struct $name:ident) => {
        #[doc = "A reference to a wasm object in an inputted module"]
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name (u32);

        impl $name {
            pub fn as_usize(&self) -> usize {
                usize::try_from(self.0).expect("16-bit CPU architectures are unsupported")
            }
        }

        impl From<usize> for $name {
            fn from(val: usize) -> Self {
                Self(u32::try_from(val).expect("only 32-bit GPU word sizes are supported, and given wasm module had more than 4GB of objects"))
            }
        }

        impl From<u32> for $name {
            fn from(val: u32) -> Self {
                Self(val)
            }
        }

        impl Deref for $name {
            type Target = u32;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    }
}

impl_index!(pub struct MemoryIndex);
impl_index!(pub struct TableIndex);
impl_index!(pub struct GlobalMutableIndex);
impl_index!(pub struct GlobalImmutableIndex);
impl_index!(pub struct ElementIndex);
impl_index!(pub struct DataIndex);

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum GlobalIndex {
    Mutable(GlobalMutableIndex),
    Immutable(GlobalImmutableIndex),
}

/// Data from the parsed module shared by all functions, e.g. function types
#[derive(Debug)]
pub struct FunctionModuleData {
    pub types: Vec<wasmparser::FuncType>,
}

/// All data for each function in the module, without imports
pub struct FuncData<'a> {
    pub ty: FuncType,
    pub locals: Vec<(u32, ValType)>,
    pub operators: Vec<OperatorByProposal<'a>>,
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
pub struct FuncUnit<'a> {
    pub data: FuncData<'a>,
    /// Pointers into the instantiated module set of all accessible objects
    pub accessible: Arc<FuncAccessible>,
}

pub struct FuncsInstance<'a> {
    pub wasm_functions: Vec<FuncUnit<'a>>,
}

impl<'a> FuncsInstance<'a> {
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

    pub fn all_items<'b>(&'b self) -> Vec<(FuncRef, &'b FuncUnit)> {
        self.wasm_functions
            .iter()
            .enumerate()
            .map(|(ptr, value)| {
                (
                    FuncRef::try_from(
                        u32::try_from(ptr).expect("64-bit GPU word sizes are unsupported"),
                    )
                    .expect("cannot have more than u32::MAX - 1 functions"),
                    value,
                )
            })
            .collect()
    }

    pub fn get(&self, ptr: FuncRef) -> Option<&FuncUnit> {
        self.wasm_functions
            .get(usize::try_from(ptr.as_u32()?).expect("16-bit CPU architectures are unsupported"))
    }
}
