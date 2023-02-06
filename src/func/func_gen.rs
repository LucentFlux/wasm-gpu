mod basic_block_gen;
mod block_gen;
mod body_gen;
mod mvp;

use std::collections::HashMap;

use itertools::Itertools;
use naga::{Handle, UniqueArena};
use wasmparser::{FuncType, ValType};

use crate::{wasm_ty_bytes, UntypedFuncPtr};

use self::body_gen::populate_body;

use super::{
    assembled_module::BuildError, bindings_gen::BindingHandles, call_graph::CallOrder, FuncUnit,
};

pub fn add_val_type(
    wgpu_ty: ValType,
    my_types: &mut UniqueArena<naga::Type>,
) -> Handle<naga::Type> {
    let name = match wgpu_ty {
        ValType::I32 => "wgpu_i32",
        ValType::I64 => "wgpu_i64",
        ValType::F32 => "wgpu_f32",
        ValType::F64 => "wgpu_f64",
        ValType::V128 => "wgpu_v128",
        ValType::FuncRef => "wgpu_func_ref",
        ValType::ExternRef => "wgpu_extern_ref",
    };

    let width = u8::try_from(wasm_ty_bytes(wgpu_ty)).expect("wasm types are small");

    let inner = match wgpu_ty {
        ValType::I32 => naga::TypeInner::Scalar {
            kind: naga::ScalarKind::Sint,
            width,
        },
        ValType::I64 => naga::TypeInner::Scalar {
            kind: naga::ScalarKind::Sint,
            width,
        },
        ValType::F32 => naga::TypeInner::Scalar {
            kind: naga::ScalarKind::Float,
            width,
        },
        ValType::F64 => naga::TypeInner::Scalar {
            kind: naga::ScalarKind::Float,
            width,
        },
        ValType::V128 => naga::TypeInner::Vector {
            size: naga::VectorSize::Quad,
            kind: naga::ScalarKind::Uint,
            width: 4,
        },
        ValType::FuncRef => naga::TypeInner::Scalar {
            kind: naga::ScalarKind::Uint,
            width,
        },
        ValType::ExternRef => naga::TypeInner::Scalar {
            kind: naga::ScalarKind::Uint,
            width,
        },
    };

    let naga_ty = naga::Type {
        name: Some(name.to_owned()),
        inner,
    };

    my_types.insert(naga_ty, naga::Span::UNDEFINED)
}

fn populate_arguments<'data>(
    ty: &FuncType,
    function: &mut naga::Function,
    my_types: &mut UniqueArena<naga::Type>,
) {
    for (i_param, param) in ty.params().into_iter().enumerate() {
        function.arguments.push(naga::FunctionArgument {
            name: None,
            ty: add_val_type(*param, my_types),
            binding: None,
        })
    }
}

fn populate_result<'data>(
    ty: &FuncType,
    function: &mut naga::Function,
    my_types: &mut UniqueArena<naga::Type>,
) -> Option<naga::Handle<naga::Type>> {
    let results = ty.results();
    if results.len() == 0 {
        function.result = None;
        return None;
    }

    let fields = results
        .into_iter()
        .map(|ty| (ty, add_val_type(*ty, my_types)))
        .collect_vec();

    let mut members = Vec::new();
    let mut offset = 0;
    for (ty, field) in fields {
        members.push(naga::StructMember {
            name: None,
            ty: field,
            binding: None,
            offset,
        });

        offset += u32::try_from(wasm_ty_bytes(*ty)).expect("wasm types are small")
    }

    let naga_ty = my_types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Struct {
                members,
                span: offset,
            },
        },
        naga::Span::UNDEFINED,
    );

    function.result = Some(naga::FunctionResult {
        ty: naga_ty,
        binding: None,
    });

    return Some(naga_ty);
}

fn populate_local_variables(
    parsed_locals: &Vec<(u32, ValType)>,
    function: &mut naga::Function,
    my_types: &mut UniqueArena<naga::Type>,
) -> HashMap<u32, Handle<naga::LocalVariable>> {
    let mut handles = HashMap::new();

    for (i_local, local_ty) in parsed_locals {
        let handle = function.local_variables.append(
            naga::LocalVariable {
                name: Some(format! {"local_{}", i_local}),
                ty: add_val_type(*local_ty, my_types),
                init: None,
            },
            naga::Span::UNDEFINED,
        );

        handles.insert(*i_local, handle);
    }

    return handles;
}

pub fn populate_base_function(
    module: &mut naga::Module,
    function_data: &FuncUnit,
    call_order: &CallOrder,
    function_to_populate: naga::Handle<naga::Function>,
    brain_function: naga::Handle<naga::Function>,
    bindings: &BindingHandles,
) -> Result<(), BuildError> {
    let FuncUnit::LocalFunction(function_data) = function_data;

    let function = module.functions.get_mut(function_to_populate);
    let types = &mut module.types;

    populate_arguments(&function_data.func_data.ty, function, types);
    let result_type = populate_result(&function_data.func_data.ty, function, types);

    let local_handles = populate_local_variables(&function_data.func_data.locals, function, types);

    populate_body(
        &function_data,
        module,
        function_to_populate,
        &local_handles,
        call_order,
        brain_function,
        bindings,
        result_type,
    )?;

    return Ok(());
}

/// Generates function that extracts arguments from buffer, calls base function,
/// then writes results to output buffer
pub fn make_entry_function(
    module: &mut naga::Module,
    func_ptr: UntypedFuncPtr,
    base_function: naga::Handle<naga::Function>,
    bindings: &BindingHandles,
) -> Result<(), BuildError> {
    return Ok(());
}
