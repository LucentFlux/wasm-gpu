use crate::compute_utils::Utils;
use crate::typed::ToRange;
use crate::{Backend, DeviceMemoryBlock, MainMemoryBlock, MemoryBlock};
use std::sync::Arc;

pub struct HostInterleavedBuffer<B, const STRIDE: usize>
where
    B: Backend,
{
    buffer: B::MainMemoryBlock,
    count: usize,
}

pub type InterleavedBufferView<'a, const STRIDE: usize> = Vec<Vec<&'a [u8; STRIDE * 4]>>;
pub type InterleavedBufferViewMut<'a, const STRIDE: usize> = Vec<Vec<&'a mut [u8; STRIDE * 4]>>;

macro_rules! impl_get {
    (
        with $self:ident on $bounds:ident
        using $as_slice:ident
        and $split_array_ref:ident
    ) => {
        let period = $self.count * STRIDE * 4;

        let s_len = $self.buffer.len();
        assert_eq!(s_len % period, 0, "buffer must be cleanly divisible into ");

        let bounds = $bounds.half_open($self.buffer.len() / $self.count);
        let buffer_bounds = (bounds.start * period)..(bounds.end * period);

        assert!(buffer_bounds.end <= s_len);

        let mut s = $self.buffer.$as_slice(buffer_bounds).await;

        assert_eq!(s.len() % period, 0);

        let mut interpretations = vec![Vec::new(); $self.count];
        while !s.is_empty() {
            for i in 0..$self.count {
                let (lhs, rhs) = s.$split_array_ref();

                interpretations.get_mut(i).unwrap().push(lhs);
                s = rhs;
            }
        }

        return interpretations;
    };
}

impl<B, const STRIDE: usize> HostInterleavedBuffer<B, STRIDE>
where
    B: Backend,
{
    /// Take this interleaved buffer and move it to device memory
    pub async fn unmap(self) -> DeviceInterleavedBuffer<B, STRIDE> {
        let buffer = self.buffer.unmap().await;
        DeviceInterleavedBuffer {
            buffer,
            count: self.count,
        }
    }

    /// Takes a memory block and interprets it as an interleaved buffer.
    ///
    /// Bounds gives the bounds for each abstract interleaved buffer, in units of `STRIDE * 4` bytes
    pub async fn get<S: ToRange<usize> + Send>(&self, bounds: S) -> InterleavedBufferView<STRIDE> {
        impl_get!(
            with self on bounds
            using as_slice
            and split_array_ref
        );
    }

    /// Takes a memory block and interprets it as a mutable interleaved buffer.
    ///
    /// Bounds gives the bounds for each abstract interleaved buffer, in units of `STRIDE * 4` bytes
    pub async fn get_mut<S: ToRange<usize> + Send>(
        &mut self,
        bounds: S,
    ) -> InterleavedBufferViewMut<STRIDE> {
        impl_get!(
            with self on bounds
            using as_slice_mut
            and split_array_mut
        );
    }
}

pub struct DeviceInterleavedBuffer<B, const STRIDE: usize>
where
    B: Backend,
{
    buffer: B::DeviceMemoryBlock,
    count: usize,
}

/// STRIDE is the length, in u32s (4 bytes), of each contiguous block of memory for each interleaved item
impl<B, const STRIDE: usize> DeviceInterleavedBuffer<B, STRIDE>
where
    B: Backend,
{
    const _A1: () = assert!(STRIDE > 0, "Stride must be non-zero");

    /// Takes a source buffer and duplicates it count times
    pub async fn new_interleaved_from(
        backend: Arc<B>,
        source: &B::DeviceMemoryBlock,
        count: usize,
    ) -> Self {
        assert!(count > 0, "Count must be non-zero");

        let len = source.len().await * count;
        let mut buffer = backend.create_device_memory_block(len, None);

        backend
            .get_utils()
            .interleave::<STRIDE>(source, &mut buffer, count)
            .await;

        Self { buffer, count }
    }

    /// Take this interleaved buffer and move it to main memory
    pub async fn map(self) -> HostInterleavedBuffer<B, STRIDE> {
        let buffer = self.buffer.map().await;
        HostInterleavedBuffer {
            buffer,
            count: self.count,
        }
    }
}
