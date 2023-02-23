mod basic_block_gen;
mod block_gen;
mod body_gen;
pub(crate) mod building;
mod mvp;

use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use itertools::Itertools;
use naga::Handle;
use wasm_types::{FuncRef, ValTypeByteCount};
use wasmparser::{FuncType, ValType};

use crate::{naga_expr, IO_ARGUMENT_ALIGNMENT_WORDS, IO_INVOCATION_ALIGNMENT_WORDS};

use self::body_gen::populate_body;

use super::{
    assembled_module::{build, BuildError, WorkingModule},
    bindings_gen::BindingHandles,
    func::FuncUnit,
};

pub fn get_entry_name(funcref: FuncRef) -> String {
    format!(
        "__wasm_entry_function_{}",
        funcref.as_u32().unwrap_or(u32::MAX)
    )
}

// The values used when building a function
pub(crate) trait WorkingFunction<'a>: Deref<Target = WorkingModule<'a>> + DerefMut {
    fn get_fn_mut(&mut self) -> &mut naga::Function;
}

pub(crate) struct WorkingBaseFunction<'a, 'b> {
    pub(crate) working_module: &'b mut WorkingModule<'a>,
    handle: Handle<naga::Function>,
    pub(crate) invocation_index: Handle<naga::Expression>,
}

impl<'a, 'b> WorkingBaseFunction<'a, 'b> {
    pub(crate) fn new(
        working_module: &'b mut WorkingModule<'a>,
        handle: Handle<naga::Function>,
    ) -> build::Result<Self> {
        // Default arguments to permiate
        let i32_ty = working_module
            .std_objs
            .tys
            .global_invocation_id
            .get(working_module)?;
        let func_mut = working_module.module.functions.get_mut(handle.clone());
        let invocation_index = func_mut.expressions.append(
            naga::Expression::FunctionArgument(func_mut.arguments.len() as u32),
            naga::Span::UNDEFINED,
        );
        func_mut.arguments.push(naga::FunctionArgument {
            name: Some("invocation_index".to_owned()),
            ty: i32_ty,
            binding: None,
        });

        Ok(Self {
            working_module,
            handle,
            invocation_index,
        })
    }
}

impl<'a, 'b> WorkingFunction<'a> for WorkingBaseFunction<'a, 'b> {
    fn get_fn_mut(&mut self) -> &mut naga::Function {
        self.working_module
            .module
            .functions
            .get_mut(self.handle.clone())
    }
}

impl<'a, 'b> Deref for WorkingBaseFunction<'a, 'b> {
    type Target = WorkingModule<'a>;

    fn deref(&self) -> &Self::Target {
        &self.working_module
    }
}

impl<'a, 'b> DerefMut for WorkingBaseFunction<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.working_module
    }
}

pub(crate) struct WorkingEntryFunction<'a, 'b> {
    pub(crate) working_module: &'b mut WorkingModule<'a>,
    entry_index: usize,
}

impl<'a, 'b> WorkingEntryFunction<'a, 'b> {
    pub(crate) fn new(working_module: &'b mut WorkingModule<'a>, entry_index: usize) -> Self {
        Self {
            working_module,
            entry_index,
        }
    }
}

impl<'a, 'b> WorkingFunction<'a> for WorkingEntryFunction<'a, 'b> {
    fn get_fn_mut(&mut self) -> &mut naga::Function {
        &mut self
            .working_module
            .module
            .entry_points
            .get_mut(self.entry_index)
            .expect("entry points are add only")
            .function
    }
}

impl<'a, 'b> Deref for WorkingEntryFunction<'a, 'b> {
    type Target = WorkingModule<'a>;

    fn deref(&self) -> &Self::Target {
        &self.working_module
    }
}

impl<'a, 'b> DerefMut for WorkingEntryFunction<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.working_module
    }
}

pub(crate) struct WasmNagaFnArg {
    handle: naga::Handle<naga::Type>,
    i_param: usize,
    wasm_ty: ValType,
}

fn populate_arguments<'a, F: WorkingFunction<'a>>(
    working: &mut F,
    ty: &FuncType,
) -> build::Result<Vec<WasmNagaFnArg>> {
    let mut arg_tys = Vec::new();
    for (i_param, param) in ty.params().into_iter().enumerate() {
        let ty = working.get_val_type(*param)?;
        working.get_fn_mut().arguments.push(naga::FunctionArgument {
            name: Some(format!("arg{}", i_param + 1)),
            ty,
            binding: None,
        });
        arg_tys.push(WasmNagaFnArg {
            handle: ty,
            i_param,
            wasm_ty: param.clone(),
        });
    }
    return Ok(arg_tys);
}

pub(crate) struct WasmNagaFnRes {
    handle: naga::Handle<naga::Type>,
    wasm_ty: Vec<ValType>,
    word_offsets: Vec<u32>,
    words: u32,
}

fn populate_result<'a, F: WorkingFunction<'a>>(
    working: &mut F,
    ty: &FuncType,
) -> build::Result<Option<WasmNagaFnRes>> {
    let results = ty.results();
    if results.len() == 0 {
        working.get_fn_mut().result = None;
        return Ok(None);
    }

    let fields = results
        .into_iter()
        .map(|ty| Ok((ty, working.get_val_type(*ty)?)))
        .collect::<build::Result<Vec<_>>>()?;

    let mut members = Vec::new();
    let mut word_offsets = Vec::new();
    let mut offset = 0;
    for (i, (ty, field)) in fields.into_iter().enumerate() {
        members.push(naga::StructMember {
            name: Some(format!("v{}", i + 1)),
            ty: field,
            binding: None,
            offset,
        });
        debug_assert_eq!(offset % 4, 0);
        word_offsets.push(offset / 4);

        offset += u32::from(ty.byte_count()).next_multiple_of(IO_ARGUMENT_ALIGNMENT_WORDS * 4);
    }
    let offset = offset.next_multiple_of(IO_INVOCATION_ALIGNMENT_WORDS * 4);

    let naga_ty = working.module.types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Struct {
                members,
                span: offset,
            },
        },
        naga::Span::UNDEFINED,
    );

    working.get_fn_mut().result = Some(naga::FunctionResult {
        ty: naga_ty,
        binding: None,
    });

    return Ok(Some(WasmNagaFnRes {
        handle: naga_ty,
        wasm_ty: Vec::from(ty.results()),
        word_offsets,
        words: offset / 4,
    }));
}

fn populate_local_variables<'a, F: WorkingFunction<'a>>(
    working: &mut F,
    parsed_locals: &Vec<(u32, ValType)>,
) -> build::Result<HashMap<u32, Handle<naga::LocalVariable>>> {
    let mut handles = HashMap::new();

    for (i_local, local_ty) in parsed_locals {
        let ty = working.get_val_type(*local_ty)?;
        let handle = working.get_fn_mut().local_variables.append(
            naga::LocalVariable {
                name: Some(format! {"local_{}", i_local}),
                ty,
                init: None,
            },
            naga::Span::UNDEFINED,
        );

        handles.insert(*i_local, handle);
    }

    return Ok(handles);
}

pub(crate) fn populate_base_function<'a>(
    working_function: &mut WorkingBaseFunction<'a, '_>,
    function_data: &FuncUnit,
    bindings: &BindingHandles,
) -> Result<(Vec<WasmNagaFnArg>, Option<WasmNagaFnRes>), BuildError> {
    let FuncUnit::LocalFunction(function_data) = function_data;

    let arg_types = populate_arguments(working_function, &function_data.func_data.ty)?;
    let result_type = populate_result(working_function, &function_data.func_data.ty)?;

    let local_handles =
        populate_local_variables(working_function, &function_data.func_data.locals)?;

    populate_body(
        working_function,
        &function_data,
        &local_handles,
        bindings,
        &result_type,
    )?;

    return Ok((arg_types, result_type));
}

/// Generates function that extracts arguments from buffer, calls base function,
/// then writes results to output buffer
pub(crate) fn populate_entry_function(
    working: &mut WorkingEntryFunction,
    func_ptr: FuncRef,
    base_function: naga::Handle<naga::Function>,
    base_function_definition: &FuncUnit,
    bindings: &BindingHandles,
    arguments: Vec<WasmNagaFnArg>,
    result: Option<WasmNagaFnRes>,
) -> build::Result<()> {
    working.get_fn_mut().name = Some(get_entry_name(func_ptr));

    // Make parameters (builtin variables)
    let workgroups_ty = working.std_objs.tys.workgroup_argument.get(working)?;
    let func_mut = working.get_fn_mut();
    let gloabl_index_param = func_mut.expressions.append(
        naga::Expression::FunctionArgument(func_mut.arguments.len() as u32),
        naga::Span::UNDEFINED,
    );
    working.get_fn_mut().arguments.push(naga::FunctionArgument {
        name: Some("global_id".to_owned()),
        ty: workgroups_ty,
        binding: Some(naga::Binding::BuiltIn(naga::BuiltIn::GlobalInvocationId)),
    });

    let instance_index = get_workgroup_index(working, gloabl_index_param);

    let arg_handles = vec![instance_index]; //read_entry_inputs(working, bindings, arguments, instance_index)?;

    // Call fn
    let call_result = result.as_ref().map(|_| {
        working.get_fn_mut().expressions.append(
            naga::Expression::CallResult(base_function),
            naga::Span::UNDEFINED,
        )
    });
    working.get_fn_mut().body.push(
        naga::Statement::Call {
            function: base_function,
            arguments: arg_handles,
            result: call_result,
        },
        naga::Span::UNDEFINED,
    );

    // Write outputs
    if let Some(call_result) = call_result {
        let results = result.expect("if call_result is Some then so is result");

        store_output(working, bindings, call_result, results, instance_index);
    }

    return Ok(());
}

fn get_workgroup_index(
    working: &mut WorkingEntryFunction,
    gloabl_index_param: naga::Handle<naga::Expression>,
) -> naga::Handle<naga::Expression> {
    naga_expr! {working => gloabl_index_param[const 0]}
}

fn read_entry_inputs(
    working: &mut WorkingEntryFunction,
    bindings: &BindingHandles,
    arguments: Vec<WasmNagaFnArg>,
    workgroup_index_param: naga::Handle<naga::Expression>,
) -> build::Result<Vec<naga::Handle<naga::Expression>>> {
    let mut arg_offset = 0;
    let mut arg_handles = Vec::new();
    for arg in &arguments {
        let load_fn = match arg.wasm_ty {
            ValType::I32 => working
                .std_objs
                .fns
                .read_i32
                .get_for(working, bindings.input.clone())?,
            _ => todo!(),
        };

        let word_id = todo!();

        let entry_fn = working.get_fn_mut();

        let arg_result = entry_fn
            .expressions
            .append(naga::Expression::CallResult(load_fn), naga::Span::UNDEFINED);
        entry_fn.body.push(
            naga::Statement::Call {
                function: load_fn,
                arguments: vec![
                    /*TODO: Calculate position in buffer of input given invocation ID */
                ],
                result: Some(arg_result.clone()),
            },
            naga::Span::UNDEFINED,
        );
        arg_handles.push(arg_result);

        arg_offset +=
            u32::from(arg.wasm_ty.byte_count()).next_multiple_of(IO_ARGUMENT_ALIGNMENT_WORDS)
    }

    return Ok(arg_handles);
}

fn store_output(
    working: &mut WorkingEntryFunction,
    bindings: &BindingHandles,
    call_result: naga::Handle<naga::Expression>,
    results: WasmNagaFnRes,
    instance_index: naga::Handle<naga::Expression>,
) -> build::Result<()> {
    let base_index = naga_expr! {working => (U32(results.words)) * instance_index};
    for (i_res, (ty, word_offset)) in results
        .wasm_ty
        .into_iter()
        .zip_eq(results.word_offsets)
        .enumerate()
    {
        let i_res = i_res as u32;
        let store_location = naga_expr! {working => base_index + (U32(word_offset))};
        let result = naga_expr! {working => call_result[const i_res]};

        let function = match ty {
            ValType::I32 => working
                .std_objs
                .fns
                .write_i32
                .get_for(working, bindings.output.clone())?,
            ValType::I64 => todo!(),
            ValType::F32 => todo!(),
            ValType::F64 => todo!(),
            ValType::V128 => todo!(),
            ValType::FuncRef => todo!(),
            ValType::ExternRef => todo!(),
        };

        working.get_fn_mut().body.push(
            naga::Statement::Call {
                function,
                arguments: vec![store_location, result],
                result: None,
            },
            naga::Span::UNDEFINED,
        );
    }

    return Ok(());
}
