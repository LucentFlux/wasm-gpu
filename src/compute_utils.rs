use crate::Backend;
use async_trait::async_trait;
use std::borrow::Cow;

macro_rules! impl_sources {
    ($( ($path:expr, $name:ident) ),* $(,)?) => {
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

impl_sources!(("compute_utils/interleave.wgsl", interleave),);

#[async_trait]
pub trait Utils<B: Backend> {
    async fn interleave<const STRIDE: usize>(
        &self,
        src: &mut B::DeviceMemoryBlock,
        dst: &mut B::DeviceMemoryBlock,
        count: usize,
    );
}
