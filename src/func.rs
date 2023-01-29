use std::marker::PhantomData;
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::FutureExt;
use perfect_derive::perfect_derive;
use wasmparser::{FuncType, ValType};

use crate::instance::data::DataPtr;
use crate::instance::element::ElementPtr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use crate::instance::table::builder::AbstractTablePtr;
use crate::{instance::global::builder::AbstractGlobalPtr, UntypedFuncPtr};
use crate::{Caller, Val, WasmTyVec};

use crate::module::operation::OperatorByProposal;

pub mod assembled_module;
mod call_graph;
pub mod func_gen;

pub trait HostFunc<T>: Send + Sync {
    fn call<'a>(
        &self,
        caller: Caller<'a, T>,
        args: Vec<Val>,
    ) -> BoxFuture<'a, anyhow::Result<Vec<Val>>>;

    fn ty(&self) -> wasmparser::FuncType;
}

/// Used for type erasure on parameters and results of function
pub struct TypedHostFn<F, T, Params, Results> {
    func: F,
    _phantom: PhantomData<fn(T, Params) -> (T, Results)>,
}

impl<F, T, Params, Results> From<F> for TypedHostFn<F, T, Params, Results> {
    fn from(func: F) -> Self {
        Self {
            func,
            _phantom: PhantomData,
        }
    }
}

impl<F, T, Params, Results> HostFunc<T> for TypedHostFn<F, T, Params, Results>
where
    F: 'static + for<'b> Fn(Caller<'b, T>, Params) -> BoxFuture<'b, anyhow::Result<Results>>,
    Params: WasmTyVec + 'static,
    Results: WasmTyVec + Send + 'static,
    TypedHostFn<F, T, Params, Results>: Send + Sync,
{
    fn call<'a>(
        &self,
        caller: Caller<'a, T>,
        args: Vec<Val>,
    ) -> BoxFuture<'a, anyhow::Result<Vec<Val>>> {
        let typed_args = match Params::try_from_val_vec(&args) {
            Ok(v) => v,
            Err(e) => return Box::pin(async { Err(e) }),
        };

        return Box::pin(
            (self.func)(caller, typed_args)
                .then(async move |r| r.map(|result| result.to_val_vec())),
        );
    }

    fn ty(&self) -> wasmparser::FuncType {
        wasmparser::FuncType::new(Vec::from(Params::VAL_TYPES), Vec::from(Results::VAL_TYPES))
    }
}

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

impl<T> FuncInstance<T> {
    pub fn accessible(&self) -> &FuncAccessible<T> {
        self.accessible
            .as_ref()
            .expect("accessible values should be populated at module link time")
    }
}

/// Something that can be called, either an instance to be converted to shader code,
/// or an index of a host function.
#[perfect_derive(Debug)]
pub enum FuncUnit<T> {
    LocalFunction(FuncInstance<T>),
    HostFunction {
        host_handle: naga::Handle<Box<dyn HostFunc<T>>>,
    },
}
