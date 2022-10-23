use crate::compute_utils::Utils;
use crate::vulkano::VulkanoBackend;
use crate::Backend;
use async_trait::async_trait;

mod shaders {
    use crate::enum_sources;
    use cps::cps;

    #[cps]
    macro_rules! impl_sources {
        (@impl_one $path:expr, $name:ident) =>
        let $src_path:expr = cps::concat!("src/", $path) in
        {
            mod $name {
                vulkano_shaders::shader! {
                    ty: "compute",
                    path: $src_path
                }
            }
        };

        ($( ($path:expr, $name:ident), )*) => {
            $(
            impl_sources!(@impl_one $path, $name);
            )*
        }
    }

    enum_sources!(impl_sources);
}

pub struct VulkanoComputeUtils {}

impl VulkanoComputeUtils {}

#[async_trait]
impl<const BUFFER_SIZE: usize> Utils<VulkanoBackend<BUFFER_SIZE>> for VulkanoComputeUtils {
    async fn interleave<const STRIDE: usize>(
        &self,
        src: &mut <VulkanoBackend<BUFFER_SIZE> as Backend>::DeviceMemoryBlock,
        dst: &mut <VulkanoBackend<BUFFER_SIZE> as Backend>::DeviceMemoryBlock,
        count: usize,
    ) {
    }
}
