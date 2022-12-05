use crate::Backend;
use async_trait::async_trait;
use std::borrow::Cow;
use std::collections::HashMap;

/// Crudely pre-processed GLSL. Please make sure this doesn't become turing complete! If it does,
/// please spend some time making a nice preprocessor language for GLSL and not just a bodged mess
/// - Joe O'C :)
struct PreGLSL {
    source: Cow<'static, str>,
}

impl PreGLSL {
    fn process(&self, replacements: Vec<(String, String)>) -> String {
        // Just plain string to string replacement
        let mut res = self.source.to_string();

        for (from, to) in replacements {
            res = res.replace(from.as_str(), to.as_str());
        }

        res
    }
}

macro_rules! impl_sources {
    ($( ($path:expr, $name:ident, { $($const_name:ident : $const_type:ty),* }), )*) => {
        $(
            #[allow(non_upper_case_globals)]
            const $name: &'static str = include_str!($path);
        )*

        pub struct WGSLSources;

        impl WGSLSources {
            $(
            paste::paste! {
                pub fn [< get_ $name _source >] <$(const $const_name : $const_type),*>() -> String {
                    let unprocessed = PreGLSL { source: Cow::Borrowed($name) };

                    unprocessed.process(vec![
                        $(
                            (concat!("CONST_", stringify!($const_name)).to_string(), $const_name .to_string())
                        ),*
                    ])
                }
            }
            )*
        }
    };
}

#[macro_export]
macro_rules! enum_sources {
    ($callback:ident) => {
        $callback!(("compute_utils/interleave.preglsl", interleave, {
            STRIDE: usize
        }),);
    };
}

enum_sources!(impl_sources);

#[async_trait]
pub trait Utils<B: Backend> {
    /// Takes a source buffer and duplicates the data contained into the dest buffer in the following way:
    /// - Split the src buffer data into chunks of `STRIDE * 4` bytes
    /// - For each of these chunks, duplicate it `count` times contiguously in `dst`
    ///
    /// I.e. if `src: [u32; 4] = [1, 2, 3, 4]`, `STRIDE = 2` and `count = 3`,
    ///
    /// after running this function `dst: [u32; 12] = [1, 2, 1, 2, 1, 2, 3, 4, 3, 4, 3, 4]`
    ///
    /// # Panics
    /// This function can (and probably should) panic if any of the following are false:
    /// - `src.len() * count <= dst.count()`
    /// - `src.len() % STRIDE == 0`
    async fn interleave<const STRIDE: usize>(
        &self,
        src: &B::DeviceMemoryBlock,
        dst: &mut B::DeviceMemoryBlock,
        count: usize,
    );
}
