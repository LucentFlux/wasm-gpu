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

pub struct InterleavedBufferView<'a>(Vec<Vec<&'a [u8]>>);
pub struct InterleavedBufferViewMut<'a>(Vec<Vec<&'a mut [u8]>>);

impl<'a> InterleavedBufferView<'a> {
    pub fn get<'b: 'a>(&'b self, index: usize) -> Option<impl Iterator<Item = &'a u8> + 'a> {
        self.0
            .get(index)
            .map(|v| v.into_iter().map(|v2| v2.into_iter()).flatten())
    }
}

impl<'a> InterleavedBufferViewMut<'a> {
    pub fn get<'b: 'a>(
        &'b mut self,
        index: usize,
    ) -> Option<impl Iterator<Item = &'a mut u8> + 'a> {
        self.0
            .get_mut(index)
            .map(|v| v.into_iter().map(|v2| v2.into_iter()).flatten())
    }

    pub fn take(self, index: usize) -> Option<impl Iterator<Item = &'a mut u8> + 'a> {
        self.0
            .into_iter()
            .skip(index)
            .next()
            .map(|v| v.into_iter().map(|v2| v2.into_iter()).flatten())
    }
}

macro_rules! impl_get {
    (
        $ret:path,
        with $self:ident on $bounds:ident
        using $as_slice:ident
        and $split_ref:ident
    ) => {{
        let period = $self.count * STRIDE * 4;

        let s_len = $self.buffer.len();
        assert_eq!(
            s_len % period,
            0,
            "buffer must be cleanly divisible into period"
        );

        let bounds = $bounds.half_open($self.buffer.len() / $self.count);
        let buffer_bounds = (bounds.start * period)..(bounds.end * period);

        if buffer_bounds.end > s_len {
            Ok($ret(vec![]))
        } else {
            let mut s = $self.buffer.$as_slice(buffer_bounds).await?;

            assert_eq!(s.len() % period, 0);

            let mut interpretations = Vec::new();
            interpretations.reserve($self.count);
            for _ in 0..$self.count {
                interpretations.push(Vec::new());
            }
            while !s.is_empty() {
                for i in 0..$self.count {
                    let (lhs, rhs) = s.$split_ref(STRIDE);

                    interpretations.get_mut(i).unwrap().push(lhs);
                    s = rhs;
                }
            }

            Ok($ret(interpretations))
        }
    }};
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
    pub async fn get<S: ToRange<usize> + Send>(
        &self,
        bounds: S,
    ) -> Result<
        InterleavedBufferView,
        <<B as Backend>::MainMemoryBlock as MainMemoryBlock<B>>::SliceError,
    > {
        return impl_get!(InterleavedBufferView,
            with self on bounds
            using as_slice
            and split_at
        );
    }

    /// Takes a memory block and interprets it as a mutable interleaved buffer.
    ///
    /// Bounds gives the bounds for each abstract interleaved buffer, in units of `STRIDE * 4` bytes
    pub async fn get_mut<S: ToRange<usize> + Send>(
        &mut self,
        bounds: S,
    ) -> Result<
        InterleavedBufferViewMut,
        <<B as Backend>::MainMemoryBlock as MainMemoryBlock<B>>::SliceError,
    > {
        return impl_get!(InterleavedBufferViewMut,
            with self on bounds
            using as_slice_mut
            and split_at_mut
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
    ) -> Result<Self, B::BufferCreationError> {
        assert!(count > 0, "Count must be non-zero");

        let len = source.len() * count;
        let mut buffer = backend.try_create_device_memory_block(len, None)?;

        backend
            .get_utils()
            .interleave::<STRIDE>(source, &mut buffer, count)
            .await;

        Ok(Self { buffer, count })
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
