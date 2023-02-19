use crate::assembled_module::{build, BuildError, WorkingModule};
use wasm_types::{ExternRef, FuncRef, Ieee32, Ieee64, V128};

use super::{TyGen, WasmTyGen};

pub(crate) struct WasmNagaI32 {}
impl TyGen for WasmNagaI32 {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Sint,
                width: 4,
            },
        };

        Ok(working.module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for WasmNagaI32 {
    type WasmTy = i32;

    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut WorkingModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(working.module.constants.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner: naga::ConstantInner::Scalar {
                    width: 4,
                    value: naga::ScalarValue::Sint(value.into()),
                },
            },
            naga::Span::UNDEFINED,
        ))
    }
}

pub(crate) struct WasmNagaF32 {}
impl TyGen for WasmNagaF32 {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Float,
                width: 4,
            },
        };

        Ok(working.module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for WasmNagaF32 {
    type WasmTy = Ieee32;

    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut WorkingModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(working.module.constants.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner: naga::ConstantInner::Scalar {
                    width: 4,
                    value: naga::ScalarValue::Float(value.to_float().into()),
                },
            },
            naga::Span::UNDEFINED,
        ))
    }
}

pub(crate) struct WasmNagaI64 {}
impl TyGen for WasmNagaI64 {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: Some("i64".to_owned()),
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Bi,
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(working.module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for WasmNagaI64 {
    type WasmTy = i64;

    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut WorkingModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        let inner = naga::ConstantInner::Composite {
            ty: ty.clone(),
            components: (0..2)
                .map(|i_word| {
                    let word = value >> (32 * i_word);
                    let word = u32::try_from(word & 0xFFFFFFFF)
                        .expect("truncated word always fits in u32");
                    working.module.constants.append(
                        naga::Constant {
                            name: None,
                            specialization: None,
                            inner: naga::ConstantInner::Scalar {
                                width: 4,
                                value: naga::ScalarValue::Uint(word.into()),
                            },
                        },
                        naga::Span::UNDEFINED,
                    )
                })
                .collect(),
        };
        Ok(working.module.constants.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner,
            },
            naga::Span::UNDEFINED,
        ))
    }
}

pub(crate) struct WasmNagaF64 {}
impl TyGen for WasmNagaF64 {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        if !working.tuneables.hardware_supports_f64 {
            return Err(BuildError::UnsupportedTypeError {
                wasm_type: wasmparser::ValType::F64,
            });
        }

        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Float,
                width: 8,
            },
        };

        Ok(working.module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for WasmNagaF64 {
    type WasmTy = Ieee64;

    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut WorkingModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        if !working.tuneables.hardware_supports_f64 {
            return Err(BuildError::UnsupportedTypeError {
                wasm_type: wasmparser::ValType::F64,
            });
        }

        Ok(working.module.constants.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner: naga::ConstantInner::Scalar {
                    width: 8,
                    value: naga::ScalarValue::Float(value.to_float()),
                },
            },
            naga::Span::UNDEFINED,
        ))
    }
}

pub(crate) struct WasmNagaV128 {}
impl TyGen for WasmNagaV128 {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: Some("v128".to_owned()),
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Quad,
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(working.module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for WasmNagaV128 {
    type WasmTy = V128;

    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut WorkingModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        let bytes = value.to_le_bytes();
        let inner = naga::ConstantInner::Composite {
            ty,
            components: bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| {
                    let word = u32::from_le_bytes(*bytes);
                    let word = u32::try_from(word & 0xFFFFFFFF)
                        .expect("truncated word always fits in u32");
                    working.module.constants.append(
                        naga::Constant {
                            name: None,
                            specialization: None,
                            inner: naga::ConstantInner::Scalar {
                                width: 4,
                                value: naga::ScalarValue::Uint(word.into()),
                            },
                        },
                        naga::Span::UNDEFINED,
                    )
                })
                .collect(),
        };
        Ok(working.module.constants.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner,
            },
            naga::Span::UNDEFINED,
        ))
    }
}

pub(crate) struct WasmNagaFuncRef {}
impl TyGen for WasmNagaFuncRef {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: Some("FuncRef".to_owned()),
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(working.module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for WasmNagaFuncRef {
    type WasmTy = FuncRef;

    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut WorkingModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(working.module.constants.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner: naga::ConstantInner::Scalar {
                    width: 4,
                    value: naga::ScalarValue::Uint(value.as_u32().unwrap_or(u32::MAX).into()),
                },
            },
            naga::Span::UNDEFINED,
        ))
    }
}

pub(crate) struct WasmNagaExternRef {}
impl TyGen for WasmNagaExternRef {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        let naga_ty = naga::Type {
            name: Some("ExtrenRef".to_owned()),
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(working.module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }
}
impl WasmTyGen for WasmNagaExternRef {
    type WasmTy = ExternRef;

    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut WorkingModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        Ok(working.module.constants.append(
            naga::Constant {
                name: None,
                specialization: None,
                inner: naga::ConstantInner::Scalar {
                    width: 4,
                    value: naga::ScalarValue::Uint(value.as_u32().unwrap_or(u32::MAX).into()),
                },
            },
            naga::Span::UNDEFINED,
        ))
    }
}
