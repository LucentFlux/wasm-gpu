use std::marker::PhantomData;

use once_cell::sync::OnceCell;

use crate::assembled_module::{build, WorkingModule};
use wasm_types::WasmTyVal;

mod base_tys;
mod buffer_tys;
mod naga_tys;

/// A type that attaches itself to a module the first time it is requested
pub(crate) trait TyGen {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>>;
}

/// A type, linked to a wasm type, that links itself on first request
pub(crate) trait WasmTyGen: TyGen {
    type WasmTy: WasmTyVal;
    // Argument `ty` is passed in from a lazy evaluation of `Self::gen`
    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut WorkingModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>>;
}

pub(crate) struct LazyTy<I: TyGen> {
    handle: OnceCell<build::Result<naga::Handle<naga::Type>>>,
    _phantom: PhantomData<I>,
}

impl<I: TyGen> LazyTy<I> {
    pub(crate) fn new() -> Self {
        Self {
            handle: OnceCell::new(),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn get(
        &self,
        working: &mut WorkingModule,
    ) -> build::Result<naga::Handle<naga::Type>> {
        self.handle.get_or_init(|| I::gen(working)).clone()
    }
}

impl<I: WasmTyGen> LazyTy<I> {
    pub(crate) fn make_const(
        &self,
        working: &mut WorkingModule,
        value: I::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        let ty = self.get(working)?;
        I::make_const(ty, working, value)
    }
}

pub(crate) struct StdTySet {
    pub(crate) workgroup_argument: LazyTy<self::naga_tys::WorkgroupArgument>,

    pub(crate) wasm_i32: LazyTy<self::base_tys::WasmNagaI32>,
    pub(crate) wasm_i64: LazyTy<self::base_tys::WasmNagaI64>,
    pub(crate) wasm_f32: LazyTy<self::base_tys::WasmNagaF32>,
    pub(crate) wasm_f64: LazyTy<self::base_tys::WasmNagaF64>,
    pub(crate) wasm_v128: LazyTy<self::base_tys::WasmNagaV128>,
    pub(crate) wasm_func_ref: LazyTy<self::base_tys::WasmNagaFuncRef>,
    pub(crate) wasm_extern_ref: LazyTy<self::base_tys::WasmNagaExternRef>,

    pub(crate) wasm_i32_array_buffer: LazyTy<self::buffer_tys::I32ArrayBuffer>,
    pub(crate) wasm_flags_buffer: LazyTy<self::buffer_tys::FlagsBuffer>,
}

impl StdTySet {
    pub(crate) fn new() -> Self {
        Self {
            workgroup_argument: LazyTy::new(),

            wasm_i32: LazyTy::new(),
            wasm_i64: LazyTy::new(),
            wasm_f32: LazyTy::new(),
            wasm_f64: LazyTy::new(),
            wasm_v128: LazyTy::new(),
            wasm_func_ref: LazyTy::new(),
            wasm_extern_ref: LazyTy::new(),

            wasm_i32_array_buffer: LazyTy::new(),
            wasm_flags_buffer: LazyTy::new(),
        }
    }
}
