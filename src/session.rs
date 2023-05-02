use std::sync::Arc;

use crate::instance::func::UntypedFuncPtr;
use crate::DeviceStoreSet;
use futures::{future::BoxFuture, FutureExt};
use wasm_gpu_funcgen::{
    u32_to_trap, CONSTANTS_BINDING_INDEX, CONSTANTS_LEN_BYTES, FLAGS_LEN_BYTES,
    IO_ARGUMENT_ALIGNMENT_WORDS, IO_INVOCATION_ALIGNMENT_WORDS, STACK_LEN_BYTES,
    TOTAL_INVOCATIONS_CONSTANT_INDEX, TRAP_FLAG_INDEX,
};
use wasm_gpu_funcgen::{
    DATA_BINDING_INDEX, ELEMENTS_BINDING_INDEX, FLAGS_BINDING_INDEX,
    IMMUTABLE_GLOBALS_BINDING_INDEX, INPUT_BINDING_INDEX, MEMORY_BINDING_INDEX,
    MUTABLE_GLOBALS_BINDING_INDEX, OUTPUT_BINDING_INDEX, STACK_BINDING_INDEX, TABLES_BINDING_INDEX,
};
use wasm_types::{Val, ValTypeByteCount};
use wasmparser::ValType;
use wgpu::{BufferAsyncError, BufferUsages};
use wgpu_async::{AsyncBuffer, AsyncDevice, AsyncQueue, OutOfMemoryError};
use wgpu_lazybuffers::{
    LazilyMappable, LockCollection, MemoryBlockConfig, MemorySystem, UnmappedLazyBuffer,
};

pub(crate) type OutputType =
    Result<Vec<Result<Vec<Val>, wasmtime_environ::Trap>>, BufferAsyncError>;

pub struct Bindings<'a> {
    pub data: &'a wgpu::Buffer,
    pub element: &'a wgpu::Buffer,
    pub mutable_globals: &'a wgpu::Buffer,
    pub immutable_globals: &'a wgpu::Buffer,
    pub memory: &'a wgpu::Buffer,
    pub table: &'a wgpu::Buffer,

    pub flags: &'a wgpu::Buffer,
    pub input: &'a wgpu::Buffer,
    pub output: &'a wgpu::Buffer,
    pub stack: &'a wgpu::Buffer,
    pub constants: &'a wgpu::Buffer,

    /// When a buffer is empty (has size 0) we need to bind something else instead. This holds ownership of those buffers
    empty_bindings: elsa::FrozenVec<Box<wgpu::Buffer>>,
}

impl<'a> Bindings<'a> {
    fn conditionally_attach<'b>(
        &'b self,
        entries: &mut Vec<wgpu::BindGroupEntry<'b>>,
        device: &wgpu::Device,
        mut buffer: &'b wgpu::Buffer,
        binding: u32,
    ) {
        if buffer.size() == 0 {
            let new_empty_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("empty stand-in buffer"),
                size: 32,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
            buffer = self.empty_bindings.push_get(Box::new(new_empty_buffer));
        }
        entries.push(wgpu::BindGroupEntry {
            binding,
            resource: buffer.as_entire_binding(),
        })
    }
    pub(crate) fn attach(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        let mut entries = Vec::new();

        self.conditionally_attach(&mut entries, device, self.data, DATA_BINDING_INDEX);
        self.conditionally_attach(&mut entries, device, self.element, ELEMENTS_BINDING_INDEX);
        self.conditionally_attach(
            &mut entries,
            device,
            self.mutable_globals,
            MUTABLE_GLOBALS_BINDING_INDEX,
        );
        self.conditionally_attach(
            &mut entries,
            device,
            self.immutable_globals,
            IMMUTABLE_GLOBALS_BINDING_INDEX,
        );
        self.conditionally_attach(&mut entries, device, self.memory, MEMORY_BINDING_INDEX);
        self.conditionally_attach(&mut entries, device, self.table, TABLES_BINDING_INDEX);
        self.conditionally_attach(&mut entries, device, self.flags, FLAGS_BINDING_INDEX);
        self.conditionally_attach(&mut entries, device, self.input, INPUT_BINDING_INDEX);
        self.conditionally_attach(&mut entries, device, self.output, OUTPUT_BINDING_INDEX);
        self.conditionally_attach(&mut entries, device, self.stack, STACK_BINDING_INDEX);
        self.conditionally_attach(
            &mut entries,
            device,
            self.constants,
            CONSTANTS_BINDING_INDEX,
        );

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &entries,
        })
    }
}

pub struct Session<'a> {
    stores: &'a mut DeviceStoreSet,
    entry_func: UntypedFuncPtr,
    args: Vec<Vec<Val>>,
}

impl<'a> Session<'a> {
    pub fn new(
        stores: &'a mut DeviceStoreSet,
        entry_func: UntypedFuncPtr,
        args: Vec<Vec<Val>>,
    ) -> Self {
        Self {
            stores,
            entry_func,
            args,
        }
    }

    async fn make_inputs(
        &self,
        device: &AsyncDevice,
        label: &str,
    ) -> Result<AsyncBuffer, OutOfMemoryError> {
        let mut data = Vec::new();
        for input_set in &self.args {
            for input in input_set {
                data.append(&mut Val::to_bytes(input));

                while data.len() % (IO_ARGUMENT_ALIGNMENT_WORDS * 4) as usize != 0 {
                    data.push(0u8)
                }
            }
            while data.len() % (IO_INVOCATION_ALIGNMENT_WORDS * 4) as usize != 0 {
                data.push(0u8)
            }
        }

        let input_buffer = device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: data.len() as u64,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: true,
            })
            .await?;

        input_buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(&data);

        input_buffer.unmap();

        Ok(input_buffer)
    }

    fn output_instance_len<'b>(output_tys: impl IntoIterator<Item = &'b ValType>) -> u64 {
        let output_length: u64 = output_tys
            .into_iter()
            .map(|res| {
                let bs = u64::from(res.byte_count());
                bs.next_multiple_of(u64::from(IO_ARGUMENT_ALIGNMENT_WORDS * 4))
            })
            .sum();
        output_length.next_multiple_of(u64::from(IO_INVOCATION_ALIGNMENT_WORDS * 4))
    }

    async fn make_output(
        &self,
        memory_system: &MemorySystem,
        label: &str,
    ) -> Result<UnmappedLazyBuffer, OutOfMemoryError> {
        let output_length = Self::output_instance_len(self.entry_func.ty().results());
        let output_length =
            self.args.len() * usize::try_from(output_length).expect("that's a big type");

        memory_system
            .try_create_device_memory_block(&MemoryBlockConfig {
                usages: BufferUsages::STORAGE,
                size: output_length,
                initial_data: None,
                locking_size: output_length,
                transfer_size: 4096,
                label,
            })
            .await
    }

    async fn make_flags(
        &self,
        memory_system: &MemorySystem,
        label: &str,
    ) -> Result<UnmappedLazyBuffer, OutOfMemoryError> {
        let flags_length = self.args.len()
            * usize::try_from(FLAGS_LEN_BYTES).expect("that's a very big wasm module");

        memory_system
            .try_create_device_memory_block(&MemoryBlockConfig {
                usages: BufferUsages::STORAGE,
                size: flags_length,
                initial_data: None,
                locking_size: flags_length,
                transfer_size: 4096,
                label,
            })
            .await
    }

    async fn make_stack(
        &self,
        device: &AsyncDevice,
        label: &str,
    ) -> Result<AsyncBuffer, OutOfMemoryError> {
        let stack_length = u64::from(STACK_LEN_BYTES)
            * u64::try_from(self.args.len()).expect("that's a too big wasm module");

        device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: stack_length,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            })
            .await
    }

    async fn make_constants(
        &self,
        device: &AsyncDevice,
        label: &str,
        count: u32,
    ) -> Result<AsyncBuffer, OutOfMemoryError> {
        let buffer = device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: CONSTANTS_LEN_BYTES as u64,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: true,
            })
            .await?;

        // Write values
        let count_loc = TOTAL_INVOCATIONS_CONSTANT_INDEX as u64 * 4;
        buffer
            .slice(count_loc..count_loc + 4)
            .get_mapped_range_mut()
            .copy_from_slice(&u32::to_le_bytes(count));

        buffer.unmap();

        return Ok(buffer);
    }

    async fn extract_output(
        ret_ty: Vec<ValType>,
        len: usize,
        flags: Arc<UnmappedLazyBuffer>,
        output: Arc<UnmappedLazyBuffer>,
        queue: AsyncQueue,
    ) -> OutputType {
        let mut results = Vec::new();

        let flags = Arc::try_unwrap(flags)
            .expect("once extraction is to be performed, all other references are done with");
        let output = Arc::try_unwrap(output).expect("see above");

        let flags = flags.map_lazy();
        let output = output.map_lazy();

        let mut flags_lock_collection = LockCollection::empty();
        let mut output_lock_collection = LockCollection::empty();

        flags.lock_reading(.., &mut flags_lock_collection).await;
        output.lock_reading(.., &mut output_lock_collection).await;

        let flags_len = usize::try_from(FLAGS_LEN_BYTES).expect("flags len is set at compile time");
        let output_len = usize::try_from(Self::output_instance_len(&ret_ty))
            .expect("instances output must fit in memory");

        for i in 0..len {
            let flags_offset = flags_len * i;
            let mut output_offset = output_len * i;

            // Extract trap flag
            let trap_flag_offset = usize::try_from(TRAP_FLAG_INDEX)
                .expect("trap flag is set at compile time to be small")
                + flags_offset;
            let trap_bytes = &flags
                .try_read_slice_with_locks(
                    &queue,
                    trap_flag_offset..trap_flag_offset + 4,
                    &flags_lock_collection,
                )
                .await?;
            let trap_bytes =
                <[u8; 4]>::try_from(trap_bytes.as_slice()).expect("there are 4 bytes to a u32");
            let trap_word = u32::from_le_bytes(trap_bytes);

            let trap = u32_to_trap(trap_word);

            if let Some(trap) = trap {
                results.push(Err(trap));
                continue;
            }

            let mut result_values = Vec::new();
            for ty in &ret_ty {
                let byte_count = ty.byte_count() as usize;
                let result_bytes = &output
                    .try_read_slice_with_locks(
                        &queue,
                        output_offset..output_offset + byte_count,
                        &output_lock_collection,
                    )
                    .await?;

                output_offset +=
                    byte_count.next_multiple_of(IO_ARGUMENT_ALIGNMENT_WORDS as usize * 4);

                let ret = ty.try_from_bytes(result_bytes).expect(&format!(
                    "returned value was not a valid {:?}, with bytes {:?}",
                    ty, result_bytes
                ));

                result_values.push(ret);
            }
            results.push(Ok(result_values));
        }

        return Ok(results);
    }

    pub async fn run(
        self,
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
    ) -> Result<BoxFuture<'a, OutputType>, OutOfMemoryError> {
        let count = self.args.len();

        let input = self
            .make_inputs(
                queue.device(),
                &format!("{}_input_buffer", self.stores.label),
            )
            .await?;
        let output = Arc::new(
            self.make_output(
                memory_system,
                &format!("{}_output_buffer", self.stores.label),
            )
            .await?,
        );
        let flags = Arc::new(
            self.make_flags(
                memory_system,
                &format!("{}_flags_buffer", self.stores.label),
            )
            .await?,
        );
        let stack = self
            .make_stack(
                queue.device(),
                &format!("{}_stack_buffer", self.stores.label),
            )
            .await?;
        let constants = Arc::new(
            self.make_constants(
                queue.device(),
                &format!("{}_constants_buffer", self.stores.label),
                u32::try_from(count).map_err(|source| OutOfMemoryError {
                    source: Box::new(source),
                })?,
            )
            .await?,
        );

        let bindings = Bindings {
            data: self.stores.datas.buffer(),
            element: self.stores.elements.buffer(),
            immutable_globals: self.stores.immutable_globals.buffer(),
            mutable_globals: self.stores.owned.mutable_globals.buffer(),
            memory: self.stores.owned.memories.buffer(),
            table: self.stores.owned.tables.buffer(),
            flags: &flags,
            input: &input,
            output: &output,
            stack: &stack,
            constants: &constants,
            empty_bindings: elsa::FrozenVec::new(),
        };

        let flags = Arc::clone(&flags);
        let output = Arc::clone(&output);
        let owned_queue = queue.clone();
        let ret_ty = self
            .entry_func
            .ty()
            .results()
            .iter()
            .map(ValType::clone)
            .collect();
        // Dispatch and be ready to parse results
        let future = self
            .stores
            .shader_module
            .run_pipeline_for_fn(queue, self.entry_func.to_func_ref(), bindings, 1, 1, 1)
            .then(move |_| Self::extract_output(ret_ty, count, flags, output, owned_queue))
            .boxed();

        return Ok(future);
    }
}
