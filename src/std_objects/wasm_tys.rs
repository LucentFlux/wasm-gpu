use std::{marker::PhantomData, sync::Arc};

use wasm_types::WasmTyVal;

use crate::build;

use super::{
    std_consts::ConstGen,
    std_fns::{BufferFn, BufferFnGen, FnGen, FromInputBuffer, FromMemoryBuffer, FromOutputBuffer},
    std_tys::TyGen,
    GenerationParameters, Generator, StdObjects, StdObjectsGenerator,
};

pub(crate) mod native_f32;
pub(crate) mod native_i32;
pub(crate) mod pollyfill_extern_ref;
pub(crate) mod pollyfill_func_ref;
pub(crate) mod polyfill_f64;
pub(crate) mod polyfill_i64;
pub(crate) mod polyfill_v128;

/// The shared implementation required for all 7 WASM types
pub(crate) trait WasmTyImpl: 'static {
    type WasmTy: WasmTyVal;

    type TyGen: TyGen;
    type DefaultGen: ConstGen;
    type ReadGen: BufferFnGen;
    type WriteGen: BufferFnGen;

    fn size_bytes() -> u32;

    // Argument `ty` is passed in from a lazy evaluation of `Self::gen`
    fn make_const(
        module: &mut naga::Module,
        objects: &StdObjects,
        value: Self::WasmTy,
    ) -> build::Result<naga::Handle<naga::Constant>>;
}

#[perfect_derive::perfect_derive(Default)]
struct GetTySize<T: WasmTyImpl>(PhantomData<T>);
impl<T: WasmTyImpl> Generator for GetTySize<T> {
    type Generated = u32;

    fn gen<Ps: GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        Ok(T::size_bytes())
    }
}

type MakeConstFn<Ty: WasmTyVal> = Arc<
    Box<dyn Fn(&mut naga::Module, &StdObjects, Ty) -> build::Result<naga::Handle<naga::Constant>>>,
>;
#[perfect_derive::perfect_derive(Default)]
struct GetMakeConst<T: WasmTyImpl>(PhantomData<T>);
impl<T: WasmTyImpl> Generator for GetMakeConst<T> {
    type Generated = MakeConstFn<T::WasmTy>;

    fn gen<Ps: GenerationParameters>(
        &self,
        module: &mut naga::Module,
        others: &StdObjectsGenerator<Ps>,
    ) -> build::Result<Self::Generated> {
        Ok(Arc::new(Box::new(T::make_const)))
    }
}

super::generator_struct! {
    pub(crate) struct WasmTyInstanceGenerator<Ty: WasmTyVal, T: WasmTyImpl<WasmTy = Ty>>
        => WasmTyInstance<Ty: WasmTyVal>
    {
        pub(crate) ty: T::TyGen => naga::Handle<naga::Type>,
        pub(crate) default: T::DefaultGen => naga::Handle<naga::Constant>,

        pub(crate) size_bytes: GetTySize<T> => u32,
        pub(crate) make_const: GetMakeConst<T> => MakeConstFn<Ty>,

        pub(crate) read_input: BufferFn<T::ReadGen, FromInputBuffer> => naga::Handle<naga::Function>,
        pub(crate) write_output: BufferFn<T::WriteGen, FromOutputBuffer> => naga::Handle<naga::Function>,
        pub(crate) read_memory: BufferFn<T::ReadGen, FromMemoryBuffer> => naga::Handle<naga::Function>,
        pub(crate) write_memory: BufferFn<T::WriteGen, FromMemoryBuffer> => naga::Handle<naga::Function>,
    }

    impl<Ty: WasmTyVal, T: WasmTyImpl<WasmTy = Ty>> Generator for WasmTyInstanceGenerator<Ty, T> {
        type Generated = WasmTyInstance<Ty>;

        ...
    }
}

/// The implementation required for numerics (i32, i64, f32, f64)
/// See https://webassembly.github.io/spec/core/syntax/instructions.html#numeric-instructions
pub(crate) trait WasmNumericTyImpl: WasmTyImpl {
    type AddGen: FnGen;
}

super::generator_struct! {
    pub(crate) struct NumericTyInstanceGenerator<Ty: WasmTyVal, T: WasmNumericTyImpl<WasmTy = Ty>>
        => NumericTyInstance<Ty: WasmTyVal>
    {
        pub(crate) base: WasmTyInstanceGenerator<Ty, T> => WasmTyInstance<Ty>,

        pub(crate) add: T::AddGen => naga::Handle<naga::Function>,
    }

    impl<Ty: WasmTyVal, T: WasmNumericTyImpl<WasmTy = Ty>> Generator for NumericTyInstanceGenerator<Ty, T> {
        type Generated = NumericTyInstance<Ty>;

        ...
    }
}

fn make_64_bit_const_from_2vec32(
    ty: naga::Handle<naga::Type>,
    module: &mut naga::Module,
    value: i64,
) -> naga::Handle<naga::Constant> {
    let inner = naga::ConstantInner::Composite {
        ty: ty.clone(),
        components: (0..2)
            .map(|i_word| {
                let word = value >> (32 * i_word);
                let word =
                    u32::try_from(word & 0xFFFFFFFF).expect("truncated word always fits in u32");
                module.constants.append(
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
    module.constants.append(
        naga::Constant {
            name: None,
            specialization: None,
            inner,
        },
        naga::Span::UNDEFINED,
    )
}
