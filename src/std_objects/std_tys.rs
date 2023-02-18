use std::marker::PhantomData;

use once_cell::sync::OnceCell;

use crate::{
    func::assembled_module::{build, WorkingModule},
    WasmTyVal,
};

mod base_tys;
mod buffer_tys;
mod naga_tys;

/// A type that attaches itself to a module the first time it is requested
pub trait TyGen {
    fn gen(working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>>;
}

/// A type, linked to a wasm type, that links itself on first request
pub trait WasmTyGen: TyGen {
    type WasmTy: WasmTyVal;
    // Argument `ty` is passed in from a lazy evaluation of `Self::gen`
    fn make_const(
        ty: naga::Handle<naga::Type>,
        working: &mut WorkingModule,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>>;
}

pub struct LazyTy<I: TyGen> {
    handle: OnceCell<build::Result<naga::Handle<naga::Type>>>,
    _phantom: PhantomData<I>,
}

impl<I: TyGen> LazyTy<I> {
    pub fn new() -> Self {
        Self {
            handle: OnceCell::new(),
            _phantom: PhantomData,
        }
    }

    pub fn get(&self, working: &mut WorkingModule) -> build::Result<naga::Handle<naga::Type>> {
        self.handle.get_or_init(|| I::gen(working)).clone()
    }
}

impl<I: WasmTyGen> LazyTy<I> {
    pub fn make_const(
        &self,
        working: &mut WorkingModule,
        value: I::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        let ty = self.get(working)?;
        I::make_const(ty, working, value)
    }
}

pub struct StdTySet {
    pub workgroup_argument: LazyTy<self::naga_tys::WorkgroupArgument>,

    pub wasm_i32: LazyTy<self::base_tys::WasmNagaI32>,
    pub wasm_i64: LazyTy<self::base_tys::WasmNagaI64>,
    pub wasm_f32: LazyTy<self::base_tys::WasmNagaF32>,
    pub wasm_f64: LazyTy<self::base_tys::WasmNagaF64>,
    pub wasm_v128: LazyTy<self::base_tys::WasmNagaV128>,
    pub wasm_func_ref: LazyTy<self::base_tys::WasmNagaFuncRef>,
    pub wasm_extern_ref: LazyTy<self::base_tys::WasmNagaExternRef>,

    pub wasm_i32_array_buffer: LazyTy<self::buffer_tys::I32ArrayBuffer>,
    pub wasm_flags_buffer: LazyTy<self::buffer_tys::FlagsBuffer>,
}

impl StdTySet {
    pub fn new() -> Self {
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
