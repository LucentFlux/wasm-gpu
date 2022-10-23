use crate::Backend;
use async_trait::async_trait;
use std::borrow::Cow;

macro_rules! impl_sources {
    ($( ($path:expr, $name:ident), )*) => {
        $(
            #[allow(non_upper_case_globals)]
            const $name: &'static str = include_str!($path);
        )*

        pub struct WGSLSources {
            $(
                pub $name: Cow<'static, str>,
            )*
        }

        impl WGSLSources {
            pub fn get() -> Self {
                Self {
                    $(
                        $name: Cow::Borrowed($name),
                    )*
                }
            }
        }
    };
}

#[macro_export]
macro_rules! enum_sources {
    ($callback:ident) => {
        $callback!(("compute_utils/interleave.glsl", interleave),);
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
