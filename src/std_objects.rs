mod flags;
mod std_consts;
mod std_fns;
mod std_globals;
mod std_tys;
mod wasm_tys;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use once_cell::unsync::OnceCell;
use wasm_types::{ExternRef, FuncRef, Val, WasmTyVal, V128};
use wasmparser::ValType;
use wasmtime_environ::Trap;

use crate::std_objects::std_tys::TyGen;
use crate::{build, Tuneables};

use self::flags::TrapConstantsGen;
use self::std_fns::FromFlagsBuffer;
use self::{
    std_consts::ConstGen,
    std_fns::{BufferFnGen, FromInputBuffer, FromMemoryBuffer, FromOutputBuffer},
    std_globals::{StdBindings, StdBindingsGenerator},
    std_tys::{U32Gen, UVec3Gen},
    wasm_tys::{
        native_f32::NativeF32, native_i32::NativeI32, pollyfill_extern_ref::PolyfillExternRef,
        pollyfill_func_ref::PolyfillFuncRef, polyfill_f64::PolyfillF64, polyfill_i64::PolyfillI64,
        polyfill_v128::PolyfillV128,
    },
};

/// Something that can take the set of standard objects to be inserted into a module, and which will insert
/// itself into said module. This allows a runtime-traversal of the instantiation DAG.
///
/// This whole trait setup may seem excessive, but in essence it provides separation between every standard
/// object, allowing the set of standard objects in a wasm shader to be extended and modified without having to
/// worry about the order in which objects must be instantiated. An optimising compiler will remove nearly all of
/// this type bloat and produce the monofunction that we would have hand-written ourselves, but which would be
/// completely unmaintainable.
pub(crate) trait Generator: Default {
    type Generated: Clone;

    fn gen<Ps: GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated>;
}

/// Sometimes during development we may have multiple dependents (DAG rather than a tree), or we may
/// accidentally create a cyclic initialisation graph. Placing a generator in this struct solves both of
/// these issues by lazily only evaluating a node once, and panicking if, while generating the object, the
/// method is re-entered.
#[perfect_derive::perfect_derive(Default)]
struct LazyGenerator<I: Generator> {
    inner: I,
    generating: AtomicBool,
    result: OnceCell<build::Result<I::Generated>>,
}

impl<I: Generator> Generator for LazyGenerator<I> {
    type Generated = I::Generated;

    fn gen<Ps: GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        self.result
            .get_or_init(|| {
                if self
                    .generating
                    .fetch_or(true, std::sync::atomic::Ordering::AcqRel)
                {
                    panic!("loop detected in std objects when generating type")
                }
                self.inner.gen(module, others)
                // No need to clear self.generating since we generate once
            })
            .clone()
    }
}

impl<I: Generator> Deref for LazyGenerator<I> {
    type Target = I;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

macro_rules! generator_struct {
    (
        $vis:vis struct $generator_name:ident $(<$($generator_generic:ident : $generator_generic_ty:path),* $(,)?>)?
            => $generated_name:ident $(<$($generated_generic:ident : $generated_generic_ty:path),* $(,)?>)?
        {
            $(
                $field_vis:vis $field:ident : $generator:ty => $generated:ty
            ),* $(,)?
        }

        impl $(<$($generator_impl_generic:ident : $generator_impl_generic_ty:path),* $(,)?>)? Generator for $generator_name_2:ident $(<$($generator_generic_param:ident),* $(,)?>)? {
            type Generated = $generated_name_2:ty;

            ...
        }
    ) => {
        $vis struct $generator_name $(<$($generator_generic: $generator_generic_ty),*>)* {
            $(
                $field: crate::std_objects::LazyGenerator<$generator>
            ),*
        }

        #[perfect_derive::perfect_derive(Clone)]
        $vis struct $generated_name $(<$($generated_generic: $generated_generic_ty),*>)* {
            $(
                $field_vis $field: $generated
            ),*
        }

        impl $(<$($generator_impl_generic: $generator_impl_generic_ty),*>)? Default for $generator_name_2 $(<$($generator_generic_param),*>)? {
            fn default() -> Self {
                Self {
                    $(
                        $field: <crate::std_objects::LazyGenerator<$generator>>::default()
                    ),*
                }
            }
        }

        impl $(<$($generator_impl_generic: $generator_impl_generic_ty),*>)? Generator for $generator_name_2 $(<$($generator_generic_param),*>)? {
            type Generated = $generated_name_2;

            fn gen<InnerPs: crate::std_objects::GenerationParameters>(
                &self,
                module: &mut naga::Module,
                others: &crate::std_objects::StdObjectsGenerator<InnerPs>,
            ) -> build::Result<Self::Generated> {
                $(
                    let $field = self.$field.gen(module, others)?;
                )*

                Ok(Self::Generated {
                    $(
                        $field
                    ),*
                })
            }
        }
    };
}
use generator_struct;

/// All swappable parts of module generation
///
/// Some different implemetations are switched out based on GPU features. By representing these
/// options in the type system, we can ensure at compile time that we have covered every case.
/// The alternative is to patten match on a set of configuration values every time we generate
/// anything. This is clearly more foolproof.
pub(crate) trait GenerationParameters {
    type I32: wasm_tys::WasmNumericTyImpl<WasmTy = i32>;
    type I64: wasm_tys::WasmNumericTyImpl<WasmTy = i64>;
    type F32: wasm_tys::WasmNumericTyImpl<WasmTy = f32>;
    type F64: wasm_tys::WasmNumericTyImpl<WasmTy = f64>;
    type V128: wasm_tys::WasmTyImpl<WasmTy = V128>;
    type FuncRef: wasm_tys::WasmTyImpl<WasmTy = FuncRef>;
    type ExternRef: wasm_tys::WasmTyImpl<WasmTy = ExternRef>;
}

generator_struct! {
    pub(crate) struct StdObjectsGenerator<Ps: GenerationParameters>
        => StdObjects
    {
        pub(crate) u32: U32Gen => naga::Handle<naga::Type>,
        pub(crate) uvec3: UVec3Gen => naga::Handle<naga::Type>,

        pub(crate) word_array_buffer_ty: std_tys::WordArrayBufferGen => naga::Handle<naga::Type>,
        pub(crate) flags_buffer_ty: std_tys::FlagsBufferGen => naga::Handle<naga::Type>,

        pub(crate) trap_fn: std_fns::BufferFn<flags::TrapFnGen, FromFlagsBuffer> => naga::Handle<naga::Function>,

        pub(crate) trap_values: TrapConstantsGen => HashMap<Option<Trap>, naga::Handle<naga::Constant>>,

        pub(crate) i32: wasm_tys::NumericTyInstanceGenerator<i32, Ps::I32> => wasm_tys::NumericTyInstance<i32>,
        pub(crate) i64: wasm_tys::NumericTyInstanceGenerator<i64, Ps::I64> => wasm_tys::NumericTyInstance<i64>,
        pub(crate) f32: wasm_tys::NumericTyInstanceGenerator<f32, Ps::F32> => wasm_tys::NumericTyInstance<f32>,
        pub(crate) f64: wasm_tys::NumericTyInstanceGenerator<f64, Ps::F64> => wasm_tys::NumericTyInstance<f64>,
        pub(crate) v128: wasm_tys::WasmTyInstanceGenerator<V128, Ps::V128> => wasm_tys::WasmTyInstance<V128>,
        pub(crate) func_ref: wasm_tys::WasmTyInstanceGenerator<FuncRef, Ps::FuncRef> => wasm_tys::WasmTyInstance<FuncRef>,
        pub(crate) extern_ref: wasm_tys::WasmTyInstanceGenerator<ExternRef, Ps::ExternRef> => wasm_tys::WasmTyInstance<ExternRef>,

        pub(crate) bindings: StdBindingsGenerator => StdBindings,
    }

    impl<Ps: GenerationParameters> Generator for StdObjectsGenerator<Ps> {
        type Generated = StdObjects;

        ...
    }
}

impl<Ps: GenerationParameters> StdObjectsGenerator<Ps> {
    fn gen(&self, module: &mut naga::Module) -> build::Result<StdObjects> {
        <Self as Generator>::gen(&self, module, &self)
    }
}

macro_rules! extract_type_field {
    ($self:ident, $val_ty:ident => element.$($field_accessor:tt)*) => {
        match $val_ty {
            ValType::I32 => $self.i32.base.$($field_accessor)*,
            ValType::I64 => $self.i64.base.$($field_accessor)*,
            ValType::F32 => $self.f32.base.$($field_accessor)*,
            ValType::F64 => $self.f64.base.$($field_accessor)*,
            ValType::V128 => $self.v128.$($field_accessor)*,
            ValType::FuncRef => $self.func_ref.$($field_accessor)*,
            ValType::ExternRef => $self.extern_ref.$($field_accessor)*,
        }
    };
}

impl StdObjects {
    pub(crate) fn new<Ps: GenerationParameters>(module: &mut naga::Module) -> build::Result<Self> {
        let generator = StdObjectsGenerator::<Ps>::default();
        generator.gen(module)
    }

    pub(crate) fn from_tuneables(
        module: &mut naga::Module,
        tuneables: &Tuneables,
    ) -> build::Result<StdObjects> {
        // TODO: Support native f64 and i64
        StdObjects::new::<FullPolyfill>(module)
    }

    /// Get's a WASM val type's naga type
    pub(crate) fn get_val_type(&self, val_ty: ValType) -> naga::Handle<naga::Type> {
        extract_type_field!(self, val_ty => element.ty)
    }

    /// Get's a WASM val type's naga type
    pub(crate) fn get_val_size_bytes(&self, val_ty: ValType) -> u32 {
        extract_type_field!(self, val_ty => element.size_bytes)
    }

    /// Makes a new constant from the value
    pub(crate) fn make_wasm_constant(
        &self,
        module: &mut naga::Module,
        value: Val,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        match value {
            Val::I32(value) => (self.i32.base.make_const)(module, self, value),
            Val::I64(value) => (self.i64.base.make_const)(module, self, value),
            Val::F32(value) => (self.f32.base.make_const)(module, self, value),
            Val::F64(value) => (self.f64.base.make_const)(module, self, value),
            Val::V128(value) => (self.v128.make_const)(module, self, value),
            Val::FuncRef(value) => (self.func_ref.make_const)(module, self, value),
            Val::ExternRef(value) => (self.extern_ref.make_const)(module, self, value),
        }
    }

    pub(crate) fn get_default_value(&self, val_ty: ValType) -> naga::Handle<naga::Constant> {
        extract_type_field!(self, val_ty => element.default)
    }

    pub(crate) fn get_read_input_fn(&self, val_ty: ValType) -> naga::Handle<naga::Function> {
        extract_type_field!(self, val_ty => element.read_input)
    }

    pub(crate) fn get_write_output_fn(&self, val_ty: ValType) -> naga::Handle<naga::Function> {
        extract_type_field!(self, val_ty => element.write_output)
    }

    pub(crate) fn get_read_memory_fn(&self, val_ty: ValType) -> naga::Handle<naga::Function> {
        extract_type_field!(self, val_ty => element.read_memory)
    }

    pub(crate) fn get_write_memory_fn(&self, val_ty: ValType) -> naga::Handle<naga::Function> {
        extract_type_field!(self, val_ty => element.write_memory)
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
