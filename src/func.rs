mod typed;

use anyhow::anyhow;
use async_trait::async_trait;
use futures::future::{join_all, BoxFuture, FutureExt};
use itertools::Itertools;
use rayon::prelude::*;
use std::collections::{BinaryHeap, HashMap};
use std::future::Future;
use std::marker::PhantomData;
use std::process::Output;
use tokio::task::JoinHandle;
use wasmtime::{FuncType, Val, ValType};

use crate::session::Session;
use crate::typed::WasmTyVec;
use crate::{Backend, FuncPtr, Store, StoreSet};
pub use typed::{TypedFuncPtr, TypedMultiCallable};

pub(crate) struct ExportFunction {
    signature: String, // TODO: make this something more reasonable
}

struct TypedHostFn<F, B, T, Params, Results> {
    func: F,
    _phantom: PhantomData<fn(B, T, Params) -> (B, T, Results)>,
}

trait HostFunc<B, T>: Send + Sync
where
    B: Backend,
{
    fn call<'a>(
        &self,
        caller: Caller<'a, B, T>,
        args: Vec<Val>,
    ) -> BoxFuture<'a, anyhow::Result<Vec<Val>>>;
}

impl<F, B, T, Params, Results> HostFunc<B, T> for TypedHostFn<F, B, T, Params, Results>
where
    B: Backend,
    F: 'static + for<'b> Fn(Caller<'b, B, T>, Params) -> BoxFuture<'b, anyhow::Result<Results>>,
    Params: WasmTyVec + 'static,
    Results: WasmTyVec + 'static,
    TypedHostFn<F, B, T, Params, Results>: Send + Sync,
{
    fn call<'a>(
        &self,
        caller: Caller<'a, B, T>,
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
}

enum FuncKind<B, T>
where
    B: Backend,
{
    Export(ExportFunction),
    Host(Box<dyn HostFunc<B, T>>),
}

pub struct Func<B, T>
where
    B: Backend,
{
    kind: FuncKind<B, T>,
    ty: FuncType,
}

impl<B, T> Func<B, T>
where
    B: Backend,
{
    pub fn params(&self) -> impl ExactSizeIterator<Item = ValType> + '_ {
        return self.ty.params();
    }

    pub fn results(&self) -> impl ExactSizeIterator<Item = ValType> + '_ {
        return self.ty.results();
    }

    pub fn ty(&self) -> FuncType {
        self.ty.clone()
    }
}

impl<B, T> Func<B, T>
where
    B: Backend + 'static,
    T: 'static,
{
    pub fn wrap<Params, Results, F>(stores: &StoreSet<B, T>, func: F) -> Vec<FuncPtr<B, T>>
    where
        Params: WasmTyVec + 'static,
        Results: WasmTyVec + 'static,
        for<'b> F: Send
            + Sync
            + Fn(Caller<'b, B, T>, Params) -> BoxFuture<'b, anyhow::Result<Results>>
            + 'static,
    {
        let func = Self {
            kind: FuncKind::Host(Box::new(TypedHostFn {
                func,
                _phantom: Default::default(),
            })),
            ty: FuncType::new(
                Params::VAL_TYPES.iter().map(ValType::clone),
                Results::VAL_TYPES.iter().map(ValType::clone),
            ),
        };

        return stores.register_function(func);
    }
}

pub trait MultiCallable<'a, B, T>
where
    B: Backend,
{
    /// Entry-point method
    ///
    /// # Panics
    /// If the number of arguments is not the same as the number of functions in this set
    /// or if any of the functions reference stores not in the store set
    fn call_all(
        self,
        stores: &'a mut StoreSet<B, T>,
        args_fn: impl FnMut(&T) -> Vec<Val>,
    ) -> BoxFuture<'a, Vec<anyhow::Result<Vec<Val>>>>;
}

impl<'a, V, B: 'a, T: 'a> MultiCallable<'a, B, T> for V
where
    V: IntoIterator<Item = &'a FuncPtr<B, T>>,
    B: Backend,
{
    /// # Panics
    /// This function panics if:
    ///  - two function pointers refer to the same store
    ///  - a function refers to a store not in stores
    fn call_all(
        self,
        stores: &'a mut StoreSet<B, T>,
        mut args_fn: impl FnMut(&T) -> Vec<Val>,
    ) -> BoxFuture<'a, Vec<anyhow::Result<Vec<Val>>>> {
        let backend = stores.backend();

        // Sort based on store ID
        let funcs: Vec<_> = self.into_iter().collect();

        // Get store references
        let stores = stores.funcs_stores(funcs.clone());

        let funcs_and_args = funcs
            .into_iter()
            .zip_eq(stores)
            .enumerate()
            .map(|(i, (func, store))| {
                let arg = args_fn(store.data());
                (i, store, func, arg)
            })
            .collect_vec();

        let session = Session::new(backend, funcs_and_args);
        return session.run();
    }
}

#[macro_export]
macro_rules! for_each_function_signature {
    ($mac:ident) => {
        $mac!(0);
        $mac!(1 A1);
        $mac!(2 A1 A2);
        $mac!(3 A1 A2 A3);
        $mac!(4 A1 A2 A3 A4);
        $mac!(5 A1 A2 A3 A4 A5);
        $mac!(6 A1 A2 A3 A4 A5 A6);
        $mac!(7 A1 A2 A3 A4 A5 A6 A7);
        $mac!(8 A1 A2 A3 A4 A5 A6 A7 A8);
        $mac!(9 A1 A2 A3 A4 A5 A6 A7 A8 A9);
        $mac!(10 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10);
        $mac!(11 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11);
        $mac!(12 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12);
        $mac!(13 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13);
        $mac!(14 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14);
        $mac!(15 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14 A15);
        $mac!(16 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13 A14 A15 A16);
    };
}

/// B is the backend type,
/// T is the data associated with the store
pub struct Caller<'a, B, T>
where
    B: Backend,
{
    store: &'a mut Store<B, T>,
}

impl<B, T> Caller<'_, B, T>
where
    B: Backend,
{
    pub fn data(&self) -> &T {
        return self.store.data();
    }
}
