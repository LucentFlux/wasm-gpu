pub mod func_ir;

use futures::future::{BoxFuture, FutureExt};
use perfect_derive::perfect_derive;
use std::fmt::Debug;
use std::marker::PhantomData;
use wasmparser::{FuncType, ValType};

use crate::instance::func::{TypedFuncPtr, UntypedFuncPtr};
use crate::instance::memory::instance::MappedMemoryInstanceSet;
use crate::instance::ptrs::AbstractPtr;
use crate::instance::ModuleInstanceReferences;
use crate::store_set::HostStoreSet;
use crate::typed::{Val, WasmTyVec};
use crate::StoreSetBuilder;

#[derive(Debug)]
pub(crate) struct ExportFunction {
    signature: String, // TODO: make this something more reasonable
}

struct TypedHostFn<F, T, Params, Results> {
    func: F,
    _phantom: PhantomData<fn(T, Params) -> (T, Results)>,
}

trait HostFunc<T>: Send + Sync {
    fn call<'a>(
        &self,
        caller: Caller<'a, T>,
        args: Vec<Val>,
    ) -> BoxFuture<'a, anyhow::Result<Vec<Val>>>;
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
}

enum FuncKind<T> {
    Export(ExportFunction),
    Host(Box<dyn HostFunc<T>>),
}

impl<T> Debug for FuncKind<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Export(arg0) => f.debug_tuple("Export").field(arg0).finish(),
            Self::Host(arg0) => f
                .debug_tuple("Host")
                .field(&"anonymous dyn host func".to_owned())
                .finish(),
        }
    }
}

#[perfect_derive(Debug)]
pub struct Func<T> {
    kind: FuncKind<T>,
    ty: FuncType,
}

impl<T> Func<T> {
    pub fn params(&self) -> &[ValType] {
        return self.ty.params();
    }

    pub fn results(&self) -> &[ValType] {
        return self.ty.results();
    }

    pub fn ty(&self) -> FuncType {
        self.ty.clone()
    }
}

impl<T> Func<T>
where
    T: 'static,
{
    pub fn wrap<Params, Results, F>(
        stores: &mut StoreSetBuilder<T>,
        func: F,
    ) -> TypedFuncPtr<T, Params, Results>
    where
        Params: WasmTyVec + 'static,
        Results: WasmTyVec + Send + 'static,
        for<'b> F: Send
            + Sync
            + Fn(Caller<'b, T>, Params) -> BoxFuture<'b, anyhow::Result<Results>>
            + 'static,
    {
        let func = Self {
            kind: FuncKind::Host(Box::new(TypedHostFn {
                func,
                _phantom: Default::default(),
            })),
            ty: FuncType::new(Vec::from(Params::VAL_TYPES), Vec::from(Results::VAL_TYPES)),
        };

        let fp: UntypedFuncPtr<T> = stores.register_function(func);

        return fp.typed();
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
/// T is the data associated with the store_set
pub struct Caller<'a, T> {
    // Decomposed store
    data: &'a mut Vec<T>,
    memory: &'a mut MappedMemoryInstanceSet,

    // Info into store data
    index: usize,
    instance: &'a ModuleInstanceReferences<T>,
}

impl<'a, T> Caller<'a, T> {
    pub fn new(
        stores: &'a mut HostStoreSet<T>,
        index: usize,
        instance: &'a ModuleInstanceReferences<T>,
    ) -> Self {
        Self {
            data: &mut stores.data,
            memory: &mut stores.owned.memories,

            index,
            instance,
        }
    }

    pub fn data(&self) -> &T {
        return self.data.get(self.index).unwrap();
    }

    pub fn data_mut(&mut self) -> &mut T {
        return self.data.get_mut(self.index).unwrap();
    }

    pub async fn get_memory(&self, name: &str) -> Option<()> {
        let memptr = self.instance.get_memory_export(name).ok()?;
        let memptr = memptr.concrete(self.index);

        todo!()
    }

    pub async fn get_memory_mut(&mut self, name: &str) -> Option<()> {
        let memptr = self.instance.get_memory_export(name).ok()?;
        let memptr = memptr.concrete(self.index);

        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_lib::{gen_test_memory_string, get_backend};
    use crate::wasp;
    use crate::{block_test, imports, Config, PanicOnAny};

    macro_rules! backend_buffer_tests {
        ($($value:expr),* $(,)?) => {
        $(
            block_test!($value, test_host_func_memory_read);
        )*
        };
    }

    backend_buffer_tests!(0, 1, 7, 8, 9, 1023, 1024, 1025, 4095, 4096, 4097);

    #[inline(never)]
    async fn test_host_func_memory_read(size: usize) {
        let (memory_system, queue) = get_backend().await;

        let (expected_data, data_str) = gen_test_memory_string(size, 203571423u32);

        let mut stores_builder = StoreSetBuilder::new(&memory_system);

        let wat = format!(
            r#"
            (module
                (import "host" "read" (func $host_read))
                (export "read" (func $host_read))

                (memory (export "mem") (data "{}"))
            )
        "#,
            data_str
        );
        let wat = wat.into_bytes();
        let module = wasp::Module::new(&Config::default(), &wat).unwrap();

        let host_read = Func::wrap(
            &mut stores_builder,
            move |caller: Caller<u32>, _param: i32| {
                let expected_data = expected_data.clone();
                Box::pin(async move {
                    let mem = caller
                        .get_memory("mem")
                        .await
                        .expect("memory mem not found");

                    for (i, b) in expected_data.iter().enumerate() {
                        assert_eq!(Some(*b), mem.get(i).await.copied());
                    }

                    Ok(())
                })
            },
        );

        let instance = stores_builder
            .instantiate_module(
                &memory_system,
                &queue,
                &module,
                imports! {
                    "host": {
                        "read": host_read
                    }
                },
            )
            .await
            .expect("could not instantiate all modules");
        let module_read = instance
            .get_typed_func::<(), ()>("read")
            .expect("could not get hello function from all instances");

        let stores_builder = stores_builder.complete(&queue).await.unwrap();

        let mut stores = stores_builder
            .build(&memory_system, &queue, 0..10)
            .await
            .unwrap();

        module_read
            .call_all(&mut stores, vec![(); 10])
            .await
            .expect_all("could not call all hello functions");
    }
}
