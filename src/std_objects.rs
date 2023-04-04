use perfect_derive::perfect_derive;
use wasm_types::{ExternRef, FuncRef, Ieee32, Ieee64, WasmTyVal, V128};

use crate::assembled_module::{build, ActiveModule};

use self::{
    std_fns::{BufferFnGen, FromInputBuffer, FromMemoryBuffer, FromOutputBuffer},
    std_globals::{StdBindings, StdBindingsGenerator},
    std_tys::{U32Gen, UVec3Gen, WasmTyGen},
    wasm_tys::{
        native_f32::NativeF32, native_i32::NativeI32, pollyfill_extern_ref::PolyfillExternRef,
        pollyfill_func_ref::PolyfillFuncRef, polyfill_f64::PolyfillF64, polyfill_i64::PolyfillI64,
        polyfill_v128::PolyfillV128,
    },
};

mod std_fns;
mod std_globals;
mod std_tys;
mod wasm_tys;

pub(crate) trait Generator {
    type Generated;

    fn gen<Ps: GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> crate::assembled_module::build::Result<Self::Generated>;
}

/// Some different implemetations are switched out based on GPU features. By representing these
/// options in the type system, we can ensure at compile time that we have covered every case.
/// The alternative is to patten match on a set of configuration values every time we generate
/// anything. This is clearly more foolproof.
///
/// This struct represents an implementation for a wasm value (i32, i64, etc) that can be read,
/// written and manipulated. We then instantiate this into a `TyInstance` once we see a module.
pub(crate) trait WasmTyImpl<WasmTy: WasmTyVal> {
    type TyGen: WasmTyGen<WasmTy = WasmTy>;
    type ReadGen: BufferFnGen;
    type WriteGen: BufferFnGen;
}

/// An instantable type in a module.
#[perfect_derive(Default)]
pub(crate) struct TyInstanceGenerator<WasmTy: WasmTyVal, T: WasmTyImpl<WasmTy>> {
    pub(crate) ty: std_tys::LazyTy<T::TyGen>,
    pub(crate) read_input: std_fns::LazyBufferFn<T::ReadGen, FromInputBuffer>,
    pub(crate) write_output: std_fns::LazyBufferFn<T::WriteGen, FromOutputBuffer>,
    pub(crate) read_memory: std_fns::LazyBufferFn<T::ReadGen, FromMemoryBuffer>,
    pub(crate) write_memory: std_fns::LazyBufferFn<T::WriteGen, FromMemoryBuffer>,
}

impl<WasmTy: WasmTyVal, T: WasmTyImpl<WasmTy>> Generator for TyInstanceGenerator<WasmTy, T> {
    type Generated = TyInstance<<T::TyGen as WasmTyGen>::WasmTy>;

    fn gen<Ps: GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        let ty = self.ty.gen(module, others)?;
        let read_input = self.read_input.gen(module, others)?;
        let write_output = self.write_output.gen(module, others)?;
        let read_memory = self.read_memory.gen(module, others)?;
        let write_memory = self.write_memory.gen(module, others)?;
        let owned_ty = ty.clone();
        Ok(Self::Generated {
            make_const: Box::new(move |module, val| {
                <T::TyGen as WasmTyGen>::make_const(owned_ty, module, val)
            }),
            ty,
            read_input,
            write_output,
            read_memory,
            write_memory,
        })
    }
}

/// An instanted type in a module.
pub(crate) struct TyInstance<Ty: WasmTyVal> {
    pub(crate) ty: naga::Handle<naga::Type>,
    pub(crate) read_input: naga::Handle<naga::Function>,
    pub(crate) write_output: naga::Handle<naga::Function>,
    pub(crate) read_memory: naga::Handle<naga::Function>,
    pub(crate) write_memory: naga::Handle<naga::Function>,
    pub(crate) make_const:
        Box<dyn Fn(&mut ActiveModule, Ty) -> build::Result<naga::Handle<naga::Constant>>>,
}

/// All swappable parts of module generation
pub(crate) trait GenerationParameters {
    type I32: WasmTyImpl<i32>;
    type I64: WasmTyImpl<i64>;
    type F32: WasmTyImpl<Ieee32>;
    type F64: WasmTyImpl<Ieee64>;
    type V128: WasmTyImpl<V128>;
    type FuncRef: WasmTyImpl<FuncRef>;
    type ExternRef: WasmTyImpl<ExternRef>;
}

/// A specific lazy instantiation of standard objects to use
#[perfect_derive(Default)]
pub(crate) struct StdObjectsGenerator<Ps: GenerationParameters> {
    u32: std_tys::LazyTy<U32Gen>,
    uvec3: std_tys::LazyTy<UVec3Gen>,

    word_array_buffer_ty: std_tys::LazyTy<std_tys::WordArrayBufferGen>,
    flags_buffer_ty: std_tys::LazyTy<std_tys::FlagsBufferGen>,

    i32: TyInstanceGenerator<i32, Ps::I32>,
    i64: TyInstanceGenerator<i64, Ps::I64>,
    f32: TyInstanceGenerator<Ieee32, Ps::F32>,
    f64: TyInstanceGenerator<Ieee64, Ps::F64>,
    v128: TyInstanceGenerator<V128, Ps::V128>,
    func_ref: TyInstanceGenerator<FuncRef, Ps::FuncRef>,
    extern_ref: TyInstanceGenerator<ExternRef, Ps::ExternRef>,

    bindings: StdBindingsGenerator,
}

impl<Ps: GenerationParameters> StdObjectsGenerator<Ps> {
    fn gen(&self, module: &mut naga::Module) -> build::Result<StdObjects> {
        let u32 = self.u32.gen(module, self)?;
        let uvec3 = self.uvec3.gen(module, self)?;

        let word_array_buffer_ty = self.word_array_buffer_ty.gen(module, self)?;
        let flags_buffer_ty = self.flags_buffer_ty.gen(module, self)?;

        let i32 = self.i32.gen(module, self)?;
        let i64 = self.i64.gen(module, self)?;
        let f32 = self.f32.gen(module, self)?;
        let f64 = self.f64.gen(module, self)?;
        let v128 = self.v128.gen(module, self)?;
        let func_ref = self.func_ref.gen(module, self)?;
        let extern_ref = self.extern_ref.gen(module, self)?;

        let bindings = self.bindings.gen(module, self)?;

        Ok(StdObjects {
            u32,
            uvec3,
            word_array_buffer_ty,
            flags_buffer_ty,
            i32,
            i64,
            f32,
            f64,
            v128,
            func_ref,
            extern_ref,
            bindings,
        })
    }
}

pub(crate) struct StdObjects {
    pub(crate) u32: naga::Handle<naga::Type>,
    pub(crate) uvec3: naga::Handle<naga::Type>,

    pub(crate) word_array_buffer_ty: naga::Handle<naga::Type>,
    pub(crate) flags_buffer_ty: naga::Handle<naga::Type>,

    pub(crate) i32: TyInstance<i32>,
    pub(crate) i64: TyInstance<i64>,
    pub(crate) f32: TyInstance<Ieee32>,
    pub(crate) f64: TyInstance<Ieee64>,
    pub(crate) v128: TyInstance<V128>,
    pub(crate) func_ref: TyInstance<FuncRef>,
    pub(crate) extern_ref: TyInstance<ExternRef>,

    pub(crate) bindings: StdBindings,
}

impl StdObjects {
    pub(crate) fn new<Ps: GenerationParameters>(module: &mut naga::Module) -> build::Result<Self> {
        let generator = StdObjectsGenerator::<Ps>::default();
        generator.gen(module)
    }
}

/// Guaranteed to work on every GPU
pub(crate) struct FullPolyfill;
impl GenerationParameters for FullPolyfill {
    type I32 = NativeI32;
    type I64 = PolyfillI64;
    type F32 = NativeF32;
    type F64 = PolyfillF64;
    type V128 = PolyfillV128;
    type FuncRef = PolyfillFuncRef;
    type ExternRef = PolyfillExternRef;
}
