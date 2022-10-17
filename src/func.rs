pub mod func_ir;

use futures::future::{BoxFuture, FutureExt};
use std::marker::PhantomData;
use wasmparser::{FuncType, ValType};

use crate::instance::func::{TypedFuncPtr, UntypedFuncPtr};
use crate::instance::memory::concrete::{MemoryInstanceSet, MemoryInstanceView};
use crate::instance::memory::concrete::{MemoryInstanceViewMut, MemoryPtr};
use crate::instance::ptrs::AbstractPtr;
use crate::instance::ModuleInstance;
use crate::store_set::StoreSet;
use crate::typed::{ToRange, Val, WasmTyVec};
use crate::{Backend, StoreSetBuilder};

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
    Results: WasmTyVec + Send + 'static,
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

impl<B, T> Func<B, T>
where
    B: Backend + 'static,
    T: 'static,
{
    pub fn wrap<Params, Results, F>(
        stores: &mut StoreSetBuilder<B, T>,
        func: F,
    ) -> TypedFuncPtr<B, T, Params, Results>
    where
        Params: WasmTyVec + 'static,
        Results: WasmTyVec + Send + 'static,
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
            ty: FuncType::new(Vec::from(Params::VAL_TYPES), Vec::from(Results::VAL_TYPES)),
        };

        let fp: UntypedFuncPtr<B, T> = stores.register_function(func);

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
pub struct Caller<'a, B, T>
where
    B: Backend,
{
    // Decomposed store
    data: &'a mut Vec<T>,
    memory: &'a mut MemoryInstanceSet<B>,

    // Info into store data
    index: usize,
    instance: &'a ModuleInstance<B, T>,
}

pub struct ViewedSlice<'a, B: Backend, T> {
    ptr: MemoryPtr<B, T>,
    view: MemoryInstanceView<'a, B>,
}

impl<'a, B: Backend, T> AsRef<[&'a [u8; 4]]> for ViewedSlice<'a, B, T> {
    fn as_ref(&self) -> &[&'a [u8; 4]] {
        self.view.get(&self.ptr).unwrap()
    }
}

pub struct ViewedSliceMut<'a, B: Backend, T> {
    ptr: MemoryPtr<B, T>,
    view: MemoryInstanceViewMut<'a, B>,
}

impl<'a, B: Backend, T> AsRef<[&'a mut [u8; 4]]> for ViewedSliceMut<'a, B, T> {
    fn as_ref(&self) -> &[&'a mut [u8; 4]] {
        self.view.get(&self.ptr).unwrap()
    }
}

impl<'a, B, T> Caller<'a, B, T>
where
    B: Backend,
{
    pub fn new(
        stores: &'a mut StoreSet<B, T>,
        index: usize,
        instance: &'a ModuleInstance<B, T>,
    ) -> Self {
        Self {
            data: &mut stores.data,
            memory: &mut stores.memories,

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

    pub async fn get_memory<S: ToRange<usize> + Send>(
        &self,
        name: &str,
        bounds: S,
    ) -> Option<ViewedSlice<B, T>> {
        let memptr = self.instance.get_memory_export(name).ok()?;
        let memptr = memptr.concrete(self.index);

        let view = self.memory.view::<T, S>(bounds).await;

        // Check pointer is valid
        {
            let slice = view.get(&memptr);
            if slice.is_none() {
                return None;
            }
        }

        return Some(ViewedSlice { ptr: memptr, view });
    }

    pub async fn get_memory_mut<S: ToRange<usize> + Send>(
        &'a mut self,
        name: &str,
        bounds: S,
    ) -> Option<ViewedSliceMut<B, T>> {
        let memptr = self.instance.get_memory_export(name).ok()?;
        let memptr = memptr.concrete(self.index);

        let view = self.memory.view_mut::<T, S>(bounds).await;

        // Check pointer is valid
        {
            let slice = view.get(&memptr);
            if slice.is_none() {
                return None;
            }
        }

        return Some(ViewedSliceMut { ptr: memptr, view });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_lib::{gen_test_data, get_backend};
    use crate::wasp;
    use crate::{block_test, imports, Config, PanicOnAny};
    use itertools::Itertools;
    use paste::paste;
    use tokio::runtime::Runtime;

    macro_rules! backend_buffer_tests {
        ($($value:expr),* $(,)?) => {
        $(
            block_test!($value, test_host_func_memory_read);
        )*
        };
    }

    backend_buffer_tests!(0, 1, 7, 8, 9, 1023, 1024, 1025, 2047, 2048, 2049);

    #[inline(never)]
    async fn test_host_func_memory_read(size: usize) {
        let backend = get_backend().await;

        let expected_data = gen_test_data(size, (size * 65) as u32);

        let engine = wasp::Engine::new(backend, Config::default());

        let mut stores_builder = StoreSetBuilder::new(&engine);
        let mut data_string = "".to_owned();
        for byte in expected_data.iter() {
            data_string += format!("\\{:02x?}", byte).as_str();
        }
        let wat = r#"
            (module
                (import "host" "read" (func $host_read))
                (export "read" (func $host_read))

                (memory (export "mem") (data ""#
            .to_owned()
            + data_string.as_str()
            + r#""))
            )
        "#;
        let module = wasp::Module::new(&engine, wat.into_bytes()).unwrap();

        let host_read = Func::wrap(
            &mut stores_builder,
            move |caller: Caller<_, u32>, _param: i32| {
                let expected_data = expected_data.clone();
                let size = size.clone();
                Box::pin(async move {
                    let mem = caller
                        .get_memory("mem", ..)
                        .await
                        .expect("memory mem not found");

                    let mem = mem.as_ref().into_iter().flat_map(|v| v.into_iter());

                    for (b1, b2) in expected_data.iter().zip_eq(mem) {
                        assert_eq!(*b1, *b2);
                    }

                    return Ok(());
                })
            },
        );

        let instance = stores_builder
            .instantiate_module(
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

        let stores_builder = stores_builder.complete();

        let mut stores = stores_builder.build(0..10).await;

        module_read
            .call_all(&mut stores, |_| ())
            .await
            .expect_all("could not call all hello functions");
    }
}
