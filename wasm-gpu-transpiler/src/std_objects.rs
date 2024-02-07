mod bindings;
mod flags;
mod wasm_tys;

use std::marker::PhantomData;

use crate::typed::Val;
use naga_ext::ConstantsExt;
use wasmparser::ValType;

use crate::{
    build, FloatingPointOptions, Tuneables, CONSTANTS_LEN_BYTES, FLAGS_LEN_BYTES,
    TOTAL_INVOCATIONS_CONSTANT_INDEX, TRAP_FLAG_INDEX,
};

use self::{
    bindings::StdBindings,
    flags::TrapValuesInstance,
    wasm_tys::{
        native_f32::NativeF32, native_i32::NativeI32, pollyfill_extern_ref::PolyfillExternRef,
        pollyfill_func_ref::PolyfillFuncRef, polyfill_f64::PolyfillF64, polyfill_i64::PolyfillI64,
        polyfill_v128::PolyfillV128,
    },
};

/// Produces a struct of generated things within a module, which may each reference previously generated things
/// (using closure syntax) when being generated, as well as a trait that can be implemented to generate each of the
/// subsequent parts of the self-referential struct. Note that we mean self-referential in the Naga way, not the
/// traditional rust way - that is, we use the handles to objects within the module to generate other objects.
macro_rules! generator_struct {
    (
        $vis:vis struct $generated_name:ident $(<$($generated_generic:ident : $generated_generic_ty:path),* $(,)?>)?
            $( ( $($parameter:ident: $param_ty:ty),* $(,)? ) )? // Parameters to the whole struct generation
        {
            $(
                $field:ident : $(|$($requirement:ident),* $(,)?|)? $generated:ty
            ),* $(,)?
        } with $trait_vis:vis trait $trait_name:ident;
    ) => {
        paste::paste!{
            mod [< $generated_name:snake _gen >] {
                #[allow(unused_imports)]
                use super::*;

                $($(
                    pub(crate) type [< $parameter:camel >] = $param_ty;
                )*)*

                $(
                    pub(crate) type [< $field:camel >] = $generated;

                    pub(crate) struct [< $field:camel Requirements >] {
                        $( $(
                            pub $requirement : [< $requirement:camel >],
                        )* )*
                    }
                )*

                // Bend hygine by using field names rather than variable identifiers - the hope is that
                // the compiler removes all of the `unwrap`s involved in this method.
                #[perfect_derive::perfect_derive(Clone)]
                pub(super) struct [< Optional $generated_name >] $(<$($generated_generic: $generated_generic_ty),*>)* {
                    $(
                        pub(super) $field: Option<$generated>,
                    )*
                    $($(
                        pub(super) $parameter: Option<$param_ty>,
                    )*)*
                }
            }

            #[perfect_derive::perfect_derive(Clone)]
            $vis struct $generated_name $(<$($generated_generic: $generated_generic_ty),*>)* {
                $(
                    pub(crate) $field: $generated
                ),*
            }

            $trait_vis trait $trait_name $(<$($generated_generic: $generated_generic_ty),*>)* {
                $(
                    fn [< gen_$field >](
                        module: &mut naga::Module,
                        others: [< $generated_name:snake _gen >]::[< $field:camel Requirements >]
                    )
                        -> build::Result<[< $generated_name:snake _gen >]::[< $field:camel >]>;
                )*
            }

            impl $generated_name {
                $trait_vis fn gen_from<T: $trait_name>(module: &mut naga::Module, $( $($parameter: $param_ty,)* )* ) -> build::Result<Self> {
                    use [< $generated_name:snake _gen >]::*;

                    let mut res = [< Optional $generated_name >] {
                        $(
                            $field: None,
                        )*
                        $($(
                            $parameter: Some($parameter),
                        )*)*
                    };

                    $(
                        let params = [< $field:camel Requirements >] {
                            $($($requirement: res.$requirement.as_ref().unwrap().clone(),)*)*
                        };
                        res.$field = Some(T::[< gen_$field >](module, params)?);
                    )*

                    Ok(Self { $(
                        $field: res.$field.unwrap(),
                    )* })
                }
            }
        }
    };
}
use generator_struct;

generator_struct! {
    pub(crate) struct WasmBoolInstance
    {
        ty: naga::Handle<naga::Type>,
        const_false: naga::Handle<naga::Constant>,
        const_true: naga::Handle<naga::Constant>,
    } with trait GenWasmBool;
}

impl GenWasmBool for WasmBoolInstance {
    fn gen_ty(
        module: &mut naga::Module,
        _others: wasm_bool_instance_gen::TyRequirements,
    ) -> build::Result<wasm_bool_instance_gen::Ty> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Sint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }

    fn gen_const_false(
        module: &mut naga::Module,
        _others: wasm_bool_instance_gen::ConstFalseRequirements,
    ) -> build::Result<wasm_bool_instance_gen::ConstFalse> {
        Ok(module.constants.append_i32(0))
    }

    fn gen_const_true(
        module: &mut naga::Module,
        _others: wasm_bool_instance_gen::ConstTrueRequirements,
    ) -> build::Result<wasm_bool_instance_gen::ConstTrue> {
        Ok(module.constants.append_i32(1))
    }
}

generator_struct! {
    pub(crate) struct NagaBoolInstance
    {
        ty: naga::Handle<naga::Type>,
        const_false: naga::Handle<naga::Constant>,
        const_true: naga::Handle<naga::Constant>,
    } with trait GenNagaBool;
}

impl GenNagaBool for NagaBoolInstance {
    fn gen_ty(
        module: &mut naga::Module,
        _others: naga_bool_instance_gen::TyRequirements,
    ) -> build::Result<naga_bool_instance_gen::Ty> {
        Ok(module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Scalar {
                    kind: naga::ScalarKind::Bool,
                    width: 1,
                },
            },
            naga::Span::UNDEFINED,
        ))
    }

    fn gen_const_false(
        module: &mut naga::Module,
        _others: naga_bool_instance_gen::ConstFalseRequirements,
    ) -> build::Result<naga_bool_instance_gen::ConstFalse> {
        Ok(module.constants.append_bool(false))
    }

    fn gen_const_true(
        module: &mut naga::Module,
        _others: naga_bool_instance_gen::ConstTrueRequirements,
    ) -> build::Result<naga_bool_instance_gen::ConstTrue> {
        Ok(module.constants.append_bool(true))
    }
}

generator_struct! {
    pub(crate) struct StdObjects (fp_options: crate::FloatingPointOptions)
    {
        word: naga::Handle<naga::Type>,
        word_max: naga::Handle<naga::Constant>, // Used for overflow calculations

        instance_id: |word| naga::Handle<naga::GlobalVariable>,
        invocations_count: |word| naga::Handle<naga::GlobalVariable>,

        uvec3: naga::Handle<naga::Type>,

        word_array_buffer_ty:   |word| naga::Handle<naga::Type>,
        constants_buffer_ty:    |word| naga::Handle<naga::Type>,
        flags_ty:               |word| naga::Handle<naga::Type>,
        flags_array_buffer_ty:  |flags_ty| naga::Handle<naga::Type>,

        bindings: |constants_buffer_ty, word_array_buffer_ty, flags_array_buffer_ty| StdBindings,

        trap_values: TrapValuesInstance,
        trap_state: |word| naga::Handle<naga::GlobalVariable>,

        naga_bool: NagaBoolInstance,
        wasm_bool: WasmBoolInstance,

        i32: |word, bindings, word_max, wasm_bool, instance_id, trap_values, trap_state, fp_options| wasm_tys::I32Instance,
        i64: |word, bindings, word_max, wasm_bool, instance_id, trap_values, trap_state, fp_options| wasm_tys::I64Instance,
        f32: |word, bindings, word_max, wasm_bool, instance_id, trap_values, trap_state, fp_options, i32| wasm_tys::F32Instance,
        f64: |word, bindings, word_max, wasm_bool, instance_id, trap_values, trap_state, fp_options| wasm_tys::F64Instance,
        v128: |word, bindings, word_max, wasm_bool, instance_id, trap_values, trap_state, fp_options| wasm_tys::V128Instance,
        func_ref: |word, bindings, word_max, wasm_bool, instance_id, trap_values, trap_state, fp_options| wasm_tys::FuncRefInstance,
        extern_ref: |word, bindings, word_max, wasm_bool, instance_id, trap_values, trap_state, fp_options| wasm_tys::ExternRefInstance,
    } with trait GenStdObjects;
}

/// All swappable parts of module generation
///
/// Some different implemetations are switched out based on GPU features. By representing these
/// options in the type system, we can ensure at compile time that we have covered every case.
/// The alternative is to patten match on a set of configuration values every time we generate
/// anything. This is clearly more foolproof.
pub(crate) trait GenerationParameters {
    type I32: wasm_tys::I32Gen;
    type I64: wasm_tys::I64Gen;
    type F32: wasm_tys::F32Gen;
    type F64: wasm_tys::F64Gen;
    type V128: wasm_tys::V128Gen;
    type FuncRef: wasm_tys::FuncRefGen;
    type ExternRef: wasm_tys::ExternRefGen;
}

macro_rules! impl_gen_wasm {
    ($ty:ident) => {
        paste::paste! {
            fn [< gen_ $ty >](
                module: &mut naga::Module,
                others: std_objects_gen::[< $ty:camel Requirements >],
            ) -> build::Result<std_objects_gen::[< $ty:camel >]> {
                std_objects_gen::[< $ty:camel >]::gen_from::<Ps::[< $ty:camel >]>(
                    module,
                    others.word,
                    others.bindings,
                    others.word_max,
                    others.wasm_bool,
                    others.instance_id,
                    others.trap_values,
                    others.trap_state,
                    others.fp_options,
                )
            }
        }
    };
}

struct StdObjectsGenerator<Ps: GenerationParameters>(PhantomData<Ps>);
impl<Ps: GenerationParameters> GenStdObjects for StdObjectsGenerator<Ps> {
    fn gen_word(
        module: &mut naga::Module,
        _others: std_objects_gen::WordRequirements,
    ) -> build::Result<std_objects_gen::Word> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar {
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }

    fn gen_word_max(
        module: &mut naga::Module,
        _others: std_objects_gen::WordMaxRequirements,
    ) -> build::Result<std_objects_gen::WordMax> {
        Ok(module.constants.append(
            naga::Constant {
                name: Some("MAX_WORD".to_owned()),
                specialization: None,
                inner: naga::ConstantInner::Scalar {
                    width: 4,
                    value: naga::ScalarValue::Uint(u32::MAX as u64),
                },
            },
            naga::Span::UNDEFINED,
        ))
    }

    fn gen_uvec3(
        module: &mut naga::Module,
        _others: std_objects_gen::Uvec3Requirements,
    ) -> build::Result<std_objects_gen::Uvec3> {
        let naga_ty = naga::Type {
            name: None,
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Tri,
                kind: naga::ScalarKind::Uint,
                width: 4,
            },
        };

        Ok(module.types.insert(naga_ty, naga::Span::UNDEFINED))
    }

    fn gen_word_array_buffer_ty(
        module: &mut naga::Module,
        others: std_objects_gen::WordArrayBufferTyRequirements,
    ) -> build::Result<std_objects_gen::WordArrayBufferTy> {
        let word_array_ty = module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Array {
                    base: others.word,
                    size: naga::ArraySize::Dynamic,
                    stride: 4,
                },
            },
            naga::Span::UNDEFINED,
        );

        Ok(word_array_ty)
    }

    fn gen_constants_buffer_ty(
        module: &mut naga::Module,
        others: std_objects_gen::ConstantsBufferTyRequirements,
    ) -> build::Result<std_objects_gen::ConstantsBufferTy> {
        let constants_members = vec![naga::StructMember {
            name: Some("total_invocations".to_owned()),
            ty: others.word,
            binding: None,
            offset: TOTAL_INVOCATIONS_CONSTANT_INDEX * 4,
        }];
        let constants_ty = module.types.insert(
            naga::Type {
                name: Some("wasm_constants".to_owned()),
                inner: naga::TypeInner::Struct {
                    span: CONSTANTS_LEN_BYTES,
                    members: constants_members,
                },
            },
            naga::Span::UNDEFINED,
        );

        Ok(constants_ty)
    }

    fn gen_flags_ty(
        module: &mut naga::Module,
        others: std_objects_gen::FlagsTyRequirements,
    ) -> build::Result<std_objects_gen::FlagsTy> {
        let flag_members = vec![naga::StructMember {
            name: Some("trap_flag".to_owned()),
            ty: others.word,
            binding: None,
            offset: TRAP_FLAG_INDEX * 4,
        }];
        let flags_ty = module.types.insert(
            naga::Type {
                name: Some("wasm_flags".to_owned()),
                inner: naga::TypeInner::Struct {
                    span: FLAGS_LEN_BYTES,
                    members: flag_members,
                },
            },
            naga::Span::UNDEFINED,
        );

        Ok(flags_ty)
    }

    fn gen_flags_array_buffer_ty(
        module: &mut naga::Module,
        others: std_objects_gen::FlagsArrayBufferTyRequirements,
    ) -> build::Result<std_objects_gen::FlagsArrayBufferTy> {
        let flags_array_ty = module.types.insert(
            naga::Type {
                name: None,
                inner: naga::TypeInner::Array {
                    base: others.flags_ty,
                    size: naga::ArraySize::Dynamic,
                    stride: FLAGS_LEN_BYTES,
                },
            },
            naga::Span::UNDEFINED,
        );

        Ok(flags_array_ty)
    }

    fn gen_bindings(
        module: &mut naga::Module,
        others: std_objects_gen::BindingsRequirements,
    ) -> build::Result<std_objects_gen::Bindings> {
        StdBindings::gen(
            module,
            others.constants_buffer_ty,
            others.word_array_buffer_ty,
            others.flags_array_buffer_ty,
        )
    }

    fn gen_trap_values(
        module: &mut naga::Module,
        _others: std_objects_gen::TrapValuesRequirements,
    ) -> build::Result<std_objects_gen::TrapValues> {
        Ok(TrapValuesInstance::gen(module))
    }
    fn gen_trap_state(
        module: &mut naga::Module,
        others: std_objects_gen::TrapStateRequirements,
    ) -> build::Result<std_objects_gen::TrapState> {
        let zero = module.constants.append_u32(0);

        Ok(module.global_variables.append(
            naga::GlobalVariable {
                name: Some("trap_state".to_owned()),
                space: naga::AddressSpace::Private,
                binding: None,
                ty: others.word,
                init: Some(zero),
            },
            naga::Span::UNDEFINED,
        ))
    }
    fn gen_naga_bool(
        module: &mut naga::Module,
        _others: std_objects_gen::NagaBoolRequirements,
    ) -> build::Result<std_objects_gen::NagaBool> {
        NagaBoolInstance::gen_from::<NagaBoolInstance>(module)
    }
    fn gen_wasm_bool(
        module: &mut naga::Module,
        _others: std_objects_gen::WasmBoolRequirements,
    ) -> build::Result<std_objects_gen::WasmBool> {
        WasmBoolInstance::gen_from::<WasmBoolInstance>(module)
    }
    fn gen_instance_id(
        module: &mut naga::Module,
        others: std_objects_gen::InstanceIdRequirements,
    ) -> build::Result<std_objects_gen::InstanceId> {
        Ok(module.global_variables.append(
            naga::GlobalVariable {
                name: Some("invocation_id".to_owned()),
                space: naga::AddressSpace::Private,
                binding: None,
                ty: others.word,
                init: None,
            },
            naga::Span::UNDEFINED,
        ))
    }
    fn gen_invocations_count(
        module: &mut naga::Module,
        others: std_objects_gen::InvocationsCountRequirements,
    ) -> build::Result<std_objects_gen::InvocationsCount> {
        Ok(module.global_variables.append(
            naga::GlobalVariable {
                name: Some("invocations_count".to_owned()),
                space: naga::AddressSpace::Private,
                binding: None,
                ty: others.word,
                init: None,
            },
            naga::Span::UNDEFINED,
        ))
    }
    impl_gen_wasm! {i32}
    impl_gen_wasm! {i64}
    fn gen_f32(
        module: &mut naga::Module,
        others: std_objects_gen::F32Requirements,
    ) -> build::Result<std_objects_gen::F32> {
        std_objects_gen::F32::gen_from::<Ps::F32>(
            module,
            others.word,
            others.bindings,
            others.word_max,
            others.wasm_bool,
            others.instance_id,
            others.trap_values,
            others.trap_state,
            others.fp_options,
            others.i32.ty,
        )
    }
    impl_gen_wasm! {f64}
    impl_gen_wasm! {v128}
    impl_gen_wasm! {func_ref}
    impl_gen_wasm! {extern_ref}
}

macro_rules! extract_type_field {
    ($self:ident, $val_ty:ident => element.$($field_accessor:tt)*) => {
        match $val_ty {
            ValType::I32 => $self.i32.$($field_accessor)*,
            ValType::I64 => $self.i64.$($field_accessor)*,
            ValType::F32 => $self.f32.$($field_accessor)*,
            ValType::F64 => $self.f64.$($field_accessor)*,
            ValType::V128 => $self.v128.$($field_accessor)*,
            ValType::Ref(rty) => match rty.heap_type() {
                wasmparser::HeapType::TypedFunc(_) | wasmparser::HeapType::Func => $self.func_ref.$($field_accessor)*,
                wasmparser::HeapType::Extern => $self.extern_ref.$($field_accessor)*,
            }
        }
    };
}

impl StdObjects {
    pub(crate) fn new<Ps: GenerationParameters>(
        module: &mut naga::Module,
        fp_options: FloatingPointOptions,
    ) -> build::Result<Self> {
        StdObjects::gen_from::<StdObjectsGenerator<Ps>>(module, fp_options)
    }

    pub(crate) fn from_tuneables(
        module: &mut naga::Module,
        tuneables: &Tuneables,
    ) -> build::Result<StdObjects> {
        // TODO: Support native f64 and i64
        StdObjects::new::<FullPolyfill>(module, tuneables.fp_options)
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
        constants: &mut naga::Arena<naga::Constant>,
        value: Val,
    ) -> build::Result<naga::Handle<naga::Constant>> {
        match value {
            Val::I32(value) => (self.i32.make_const)(constants, self, value),
            Val::I64(value) => (self.i64.make_const)(constants, self, value),
            Val::F32(value) => (self.f32.make_const)(constants, self, value),
            Val::F64(value) => (self.f64.make_const)(constants, self, value),
            Val::V128(value) => (self.v128.make_const)(constants, self, value),
            Val::FuncRef(value) => (self.func_ref.make_const)(constants, self, value),
            Val::ExternRef(value) => (self.extern_ref.make_const)(constants, self, value),
        }
    }

    pub(crate) fn get_default_value(&self, val_ty: ValType) -> naga::Handle<naga::Expression> {
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
