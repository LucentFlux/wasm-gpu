use crate::compute_utils::Utils;
use crate::typed::ToRange;
use crate::{Backend, DeviceMemoryBlock, MainMemoryBlock};
use std::marker::PhantomData;
use std::sync::Arc;

pub struct HostInterleavedBuffer<B, const STRIDE: usize>
where
    B: Backend,
{
    buffer: B::MainMemoryBlock,
    count: usize,
}

impl<B, const STRIDE: usize> HostInterleavedBuffer<B, STRIDE>
where
    B: Backend,
{
    /// Take this interleaved buffer and move it to device memory
    pub async fn move_to_main_memory(self) -> DeviceInterleavedBuffer<B, STRIDE> {
        let buffer = self.buffer.move_to_device_memory().await;
        DeviceInterleavedBuffer { buffer, count }
    }

    #[doc = r#"Takes a memory block and interprets it as an interleaved buffer.
        Bounds gives the bounds for each abstract interleaved buffer, in units of `STRIDE` bytes"#]
    pub async fn get<S: ToRange<usize> + Send>(&self, bounds: S) -> Vec<Vec<&[u8; STRIDE]>> {
        let s_len = source.len().await;
        assert_eq!(s_len % (count * STRIDE), 0);

        let bounds = bounds.half_open(source.len().await / count);
        let buffer_bounds = (bounds.start * count * STRIDE)..(bounds.end * count * STRIDE);

        assert!(buffer_bounds.end <= s_len);

        let mut s = source.as_slice(buffer_bounds).await;

        debug_assert_eq!(s.len() % (count * STRIDE), 0);

        let mut interpretations = vec![Vec::new(); count];
        while !s.is_empty() {
            for i in 0..count {
                let (lhs, rhs) = s.split_array_ref();
                interpretations.get_mut(i).unwrap().push(lhs);
                s = rhs;
            }
        }

        Self {
            interpretations,
            _phantom: Default::default(),
        }
    }
}

pub struct DeviceInterleavedBuffer<B, const STRIDE: usize>
where
    B: Backend,
{
    buffer: B::DeviceMemoryBlock,
    count: usize,
}

/// STRIDE is the length, in bytes, of each contiguous block of memory for each interleaved item
impl<B, const STRIDE: usize> DeviceInterleavedBuffer<B, STRIDE>
where
    B: Backend,
{
    const _: () = assert!(STRIDE > 0, "Stride must be non-zero");
    const _: () = assert_eq!(STRIDE % 4, 0, "Stride must be a multiple of 4 bytes");

    /// Takes a source buffer and duplicates it count times
    pub async fn new_interleaved_from(
        backend: Arc<B>,
        source: &mut B::DeviceMemoryBlock,
        count: usize,
    ) -> Self {
        assert!(count > 0, "Count must be non-zero");

        let len = source.len().await * count;
        let mut buffer = backend.create_device_memory_block(len, None);

        backend
            .get_utils()
            .interleave::<{ STRIDE / 4 }>(src, &mut buffer, count)
            .await;

        Self { buffer, count }
    }

    /// Take this interleaved buffer and move it to main memory
    pub async fn move_to_main_memory(self) -> HostInterleavedBuffer<B, STRIDE> {
        let buffer = self.buffer.move_to_main_memory().await;
        HostInterleavedBuffer { buffer, count }
    }
}
