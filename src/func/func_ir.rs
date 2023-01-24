use std::collections::HashMap;

use itertools::Itertools;
use naga::{Handle, UniqueArena};
use wasmparser::{FuncType, ValType};

use crate::{module::module_environ::ParsedFunc, wasm_ty_bytes, Engine};

fn add_val_type(wgpu_ty: &ValType, my_types: &mut UniqueArena<naga::Type>) -> Handle<naga::Type> {
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

/// Intermediate Representation of a function. Used to generate SPIR-V.
#[derive(Debug)]
pub struct FuncIR {
    func: naga::Function,
}

impl FuncIR {
    fn make_arguments<'data>(
        ty: &FuncType,
        my_types: &mut UniqueArena<naga::Type>,
    ) -> Vec<naga::FunctionArgument> {
        let mut args = Vec::new();
        for (i_param, param) in ty.params().into_iter().enumerate() {
            args.push(naga::FunctionArgument {
                name: None,
                ty: add_val_type(param, my_types),
                binding: Some(naga::Binding::Location {
                    location: u32::try_from(i_param).expect("cannot have more than u32::MAX args"),
                    interpolation: None,
                    sampling: None,
                }),
            })
        }
        return args;
    }

    fn make_result<'data>(
        ty: &FuncType,
        my_types: &mut UniqueArena<naga::Type>,
    ) -> Option<naga::FunctionResult> {
        let results = ty.results();
        if results.len() == 0 {
            return None;
        }

        let fields = results
            .into_iter()
            .map(|ty| (ty, add_val_type(ty, my_types)))
            .collect_vec();

        let mut members = Vec::new();
        let mut offset = 0;
        for (ty, field) in fields {
            members.push(naga::StructMember {
                name: None,
                ty: field,
                binding: Some(naga::Binding::Location {
                    location: 0,
                    interpolation: None,
                    sampling: None,
                }),
                offset,
            });

            offset += u32::try_from(wasm_ty_bytes(ty)).expect("wasm types are small")
        }

        let naga_ty = my_types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Struct { members, span: 0 },
            },
            naga::Span::UNDEFINED,
        );

        let arg = naga::FunctionResult {
            ty: naga_ty,
            binding: Some(naga::Binding::Location {
                location: 0,
                interpolation: None,
                sampling: None,
            }),
        };
        return Some(arg);
    }

    fn make_local_variables<'data>(
        parsed: &ParsedFunc<'data>,
        my_types: &mut UniqueArena<naga::Type>,
    ) -> (
        naga::Arena<naga::LocalVariable>,
        HashMap<u32, Handle<naga::LocalVariable>>,
    ) {
        let mut locals = naga::Arena::new();
        let mut handles = HashMap::new();

        for (i_local, local_ty) in &parsed.locals {
            let handle = locals.append(
                naga::LocalVariable {
                    name: Some(format! {"local_{}", i_local}),
                    ty: add_val_type(local_ty, my_types),
                    init: None,
                },
                naga::Span::UNDEFINED,
            );

            handles.insert(*i_local, handle);
        }

        return (locals, handles);
    }

    pub fn from_wasm<'data>(
        engine: &mut Engine,
        ty: &FuncType,
        parsed: &ParsedFunc<'data>,
        function_name: String,
    ) -> Self {
        let arguments = Self::make_arguments(ty, &mut engine.my_types);
        let result = Self::make_result(ty, &mut engine.my_types);

        let (local_variables, local_handles) =
            Self::make_local_variables(parsed, &mut engine.my_types);

        Self {
            func: naga::Function {
                name: Some(function_name),
                arguments,
                result,
                local_variables,
                expressions: todo!(),
                named_expressions: naga::FastHashMap::with_hasher(Default::default()),
                body: todo!(),
            },
        }
    }
}

#[test]
fn debug_test() {}
