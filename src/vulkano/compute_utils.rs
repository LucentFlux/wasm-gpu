use crate::compute_utils::Utils;
use crate::vulkano::VulkanoBackend;
use async_trait::async_trait;

mod shaders {
    use crate::enum_sources;
    use vulkano_shaders::shader;

    macro_rules! impl_sources {
        ($( ($path:expr, $name:ident), )*) => {
            $(
            mod $name {
                shader! {
                    ty: "compute",
                    path: $path
                }
            }
            )*
        };
    }

    enum_sources!(impl_sources);
}

pub struct VulkanoComputeUtils {}

impl VulkanoComputeUtils {}

#[async_trait]
impl Utils<VulkanoBackend> for VulkanoComputeUtils {
    async fn interleave<const STRIDE: usize>(
        &self,
        src: &mut VulkanoBackend::DeviceMemoryBlock,
        dst: &mut VulkanoBackend::DeviceMemoryBlock,
        count: usize,
    ) {
    }
}
