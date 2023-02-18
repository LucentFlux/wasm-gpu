mod basic_block_gen;
mod block_gen;
mod body_gen;
pub mod building;
mod mvp;

use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use itertools::Itertools;
use naga::Handle;
use wasmparser::{FuncType, ValType};

use crate::{
    instance::memory::instance::MEMORY_STRIDE_BYTES, naga_expr, session::INVOCATION_ID_FLAG_INDEX,
    wasm_ty_bytes, UntypedFuncPtr,
};

use self::body_gen::populate_body;

use super::{
    assembled_module::{build, BuildError, WorkingModule},
    bindings_gen::BindingHandles,
    FuncUnit,
};

// The values used when building a function
pub trait WorkingFunction<'a>: Deref<Target = WorkingModule<'a>> + DerefMut {
    fn get_fn_mut(&mut self) -> &mut naga::Function;
}

pub struct WorkingBaseFunction<'a, 'b> {
    pub working_module: &'b mut WorkingModule<'a>,
    handle: Handle<naga::Function>,
}

impl<'a, 'b> WorkingBaseFunction<'a, 'b> {
    pub fn new(working_module: &'b mut WorkingModule<'a>, handle: Handle<naga::Function>) -> Self {
        Self {
            working_module,
            handle,
        }
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

pub struct WorkingEntryFunction<'a, 'b> {
    pub working_module: &'b mut WorkingModule<'a>,
    entry_index: usize,
}

impl<'a, 'b> WorkingEntryFunction<'a, 'b> {
    pub fn new(working_module: &'b mut WorkingModule<'a>, entry_index: usize) -> Self {
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

pub struct WasmNagaFnArg {
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

pub struct WasmNagaFnRes {
    handle: naga::Handle<naga::Type>,
    wasm_ty: Vec<ValType>,
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
    let mut offset = 0;
    for (i, (ty, field)) in fields.into_iter().enumerate() {
        members.push(naga::StructMember {
            name: Some(format!("v{}", i + 1)),
            ty: field,
            binding: None,
            offset,
        });

        offset += u32::try_from(wasm_ty_bytes(*ty)).expect("wasm types are small")
    }

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

pub fn populate_base_function<'a>(
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
pub fn populate_entry_function(
    working: &mut WorkingEntryFunction,
    func_ptr: UntypedFuncPtr,
    base_function: naga::Handle<naga::Function>,
    base_function_definition: &FuncUnit,
    bindings: &BindingHandles,
    arguments: Vec<WasmNagaFnArg>,
    result: Option<WasmNagaFnRes>,
) -> build::Result<()> {
    let base_func_name = func_ptr.get_entry_name();

    // Make parameters (builtin variables)
    let workgroups_ty = working.std_objs.tys.workgroup_argument.get(working)?;
    let entry_fn = working.get_fn_mut();
    let workgroup_index_param = entry_fn.expressions.append(
        naga::Expression::FunctionArgument(entry_fn.arguments.len() as u32),
        naga::Span::UNDEFINED,
    );
    entry_fn.arguments.push(naga::FunctionArgument {
        name: Some("workgroup_index".to_owned()),
        ty: workgroups_ty,
        binding: Some(naga::Binding::BuiltIn(naga::BuiltIn::WorkGroupId)),
    });

    // Calculate some constants used elsewhere
    let flags = entry_fn.expressions.append(
        naga::Expression::GlobalVariable(bindings.flags),
        naga::Span::UNDEFINED,
    );
    let invocation_id_flags_field = naga_expr!{flags[const INVOCATION_ID_FLAG_INDEX]};

    let invocation_id = todo!(); //naga_expr!(workgroup_index_param[const 0]);
    entry_fn.body.push(
        naga::Statement::Store {
            pointer: invocation_id_flags_field,
            value: invocation_id,
        },
        naga::Span::UNDEFINED,
    );

    // Make locals
    let argument_variables = arguments
        .iter()
        .enumerate()
        .map(|(i, ty)| {
            entry_fn.local_variables.append(
                naga::LocalVariable {
                    name: Some(format!("arg{}", i)),
                    init: None,
                    ty: ty.handle,
                },
                naga::Span::UNDEFINED,
            )
        })
        .collect_vec();
    let result_variable = result.as_ref().map(|ty| {
        entry_fn.local_variables.append(
            naga::LocalVariable {
                name: Some("res".to_owned()),
                init: None,
                ty: ty.handle.clone(),
            },
            naga::Span::UNDEFINED,
        )
    });

    // Convert locals to pointers
    let argument_ptrs = argument_variables
        .iter()
        .map(|res_var| {
            entry_fn.expressions.append(
                naga::Expression::LocalVariable(res_var.clone()),
                naga::Span::UNDEFINED,
            )
        })
        .collect_vec();
    let result_ptr = result_variable.as_ref().map(|res_var| {
        entry_fn.expressions.append(
            naga::Expression::LocalVariable(res_var.clone()),
            naga::Span::UNDEFINED,
        )
    });

    // Read inputs
    let input_ref = entry_fn.expressions.append(
        naga::Expression::GlobalVariable(bindings.input.clone()),
        naga::Span::UNDEFINED,
    );
    let mut arg_offset = 0;
    for (arg_ty, arg_variable_ptr) in arguments.iter().zip_eq(&argument_ptrs) {
        let load_fn = match arg_ty.wasm_ty {
            ValType::I32 => working.std_objs.fns.read_i32.get(&mut working)?,
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
                    input_ref, /*TODO: Calculate position in buffer of input given invocation ID */
                ],
                result: Some(arg_result.clone()),
            },
            naga::Span::UNDEFINED,
        );
        entry_fn.body.push(
            naga::Statement::Store {
                pointer: arg_variable_ptr.clone(),
                value: arg_result,
            },
            naga::Span::UNDEFINED,
        );

        arg_offset += wasm_ty_bytes(arg_ty.wasm_ty)
    }

    // Call fn
    entry_fn.body.push(
        naga::Statement::Call {
            function: base_function,
            arguments: argument_ptrs,
            result: result_ptr,
        },
        naga::Span::UNDEFINED,
    );

    // Write outputs
    if let Some(result) = result {
        //let store_location =
    }

    return Ok(());
}
