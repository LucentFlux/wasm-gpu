use super::build;

pub(crate) mod native_f32;
pub(crate) mod native_i32;
pub(crate) mod pollyfill_extern_ref;
pub(crate) mod pollyfill_func_ref;
pub(crate) mod polyfill_f64;
pub(crate) mod polyfill_i64;
pub(crate) mod polyfill_v128;

fn make_64_bit_const_from_2vec32(
    ty: naga::Handle<naga::Type>,
    module: &mut naga::Module,
    value: i64,
) -> build::Result<naga::Handle<naga::Constant>> {
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
    Ok(module.constants.append(
        naga::Constant {
            name: None,
            specialization: None,
            inner,
        },
        naga::Span::UNDEFINED,
    ))
}
