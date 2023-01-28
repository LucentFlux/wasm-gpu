use std::ops::RangeBounds;
use wgpu::BufferAsyncError;
use wgpu_async::AsyncQueue;

use crate::instance::memory::instance::{MappedMemoryInstanceSet, MemoryView};
use crate::instance::ptrs::AbstractPtr;
use crate::instance::ModuleInstanceReferences;
use crate::store_set::HostStoreSet;

pub struct ActiveMemoryView<'a> {
    view: MemoryView<'a>,
    queue: &'a AsyncQueue,
}

impl<'a> ActiveMemoryView<'a> {
    pub async fn try_read_slice(
        &self,
        slice: impl RangeBounds<usize>,
    ) -> Result<Vec<u8>, BufferAsyncError> {
        self.view.try_read_slice(self.queue, slice).await
    }

    pub async fn try_write_slice(
        &self,
        slice: impl RangeBounds<usize>,
        data: &[u8],
    ) -> Result<(), BufferAsyncError> {
        self.view.try_write_slice(self.queue, slice, data).await
    }
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

    // Action data
    queue: &'a AsyncQueue,
}

impl<'a, T> Caller<'a, T> {
    pub fn new(
        stores: &'a mut HostStoreSet<T>,
        index: usize,
        instance: &'a ModuleInstanceReferences<T>,
        queue: &'a AsyncQueue,
    ) -> Self {
        Self {
            data: &mut stores.data,
            memory: &mut stores.owned.memories,

            index,
            instance,

            queue,
        }
    }

    pub fn data(&self) -> &T {
        return self.data.get(self.index).unwrap();
    }

    pub fn data_mut(&mut self) -> &mut T {
        return self.data.get_mut(self.index).unwrap();
    }

    pub async fn get_memory<'b>(&'b self, name: &str) -> Option<ActiveMemoryView<'b>> {
        let memptr = self.instance.get_memory_export(name).ok()?;
        let memptr = memptr.concrete(self.index);

        let view = self.memory.get(&memptr);
        Some(ActiveMemoryView {
            view,
            queue: self.queue,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_lib::{gen_test_memory_string, get_backend};
    use crate::{block_test, imports, Config, PanicOnAny};
    use crate::{wasp, MappedStoreSetBuilder};

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
        let config = Config::default();
        let mut stores_builder = MappedStoreSetBuilder::new(&memory_system);

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
        let module = wasp::Module::new(&config, &wat, "test_module".to_owned()).unwrap();

        let host_read =
            stores_builder.register_host_function(move |caller: Caller<u32>, _param: i32| {
                let expected_data = expected_data.clone();
                Box::pin(async move {
                    let mem = caller
                        .get_memory("mem")
                        .await
                        .expect("memory mem not found");

                    let got_data = mem.try_read_slice(..).await.expect("mem not read");
                    assert_eq!(got_data, expected_data);

                    Ok(())
                })
            });

        let instance = stores_builder
            .instantiate_module(
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
