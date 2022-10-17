use crate::compute_utils::Utils;
use crate::memory::DynamicMemoryBlock;
use crate::typed::ToRange;
use crate::{Backend, MainMemoryBlock};
use std::marker::PhantomData;
use std::sync::Arc;

/// Takes an interleaved slice and locks its length, allowing concurrent accesses to non-aliasing slices
pub struct InterleavedBufferView<'a, B, const STRIDE: usize>
where
    B: Backend,
{
    interpretations: Vec<Vec<&'a [u8; STRIDE]>>,
    _phantom: PhantomData<B>,
}

/// Used to implement both the mutable and immutable variations
macro_rules! interpret {
    (
        (source: $source:ty) {
            use $as_slice:ident
            and $split_array_ref:ident
        }
    ) => {
        #[doc="Takes a memory block and interprets it as an interleaved buffer\
        Bounds gives the bounds for each abstract interleaved buffer, in units of `STRIDE` bytes\
        # Panics\
        Count cannot be 0\
        `source.len()` must be divisible by `count * STRIDE`\
        The backing buffer must have length >= `bounds.end * count * STRIDE`"]
        pub async fn interpret<S: ToRange<usize> + Send>(
            source: $source,
            bounds: S,
            count: usize,
        ) -> Self {
            assert!(count > 0);
            let s_len = source.len().await;
            assert_eq!(s_len % (count * STRIDE), 0);

            let bounds = bounds.half_open(source.len() / count);
            let buffer_bounds = (bounds.start * count * STRIDE)..(bounds.end * count * STRIDE);

            assert!(buffer_bounds.end <= s_len);

            let mut s = source.$as_slice(buffer_bounds).await;

            debug_assert_eq!(s.len() % (count * STRIDE), 0);

            let mut interpretations = vec![Vec::new(); count];
            while !s.is_empty() {
                for i in 0..count {
                    let (lhs, rhs) = s.$split_array_ref();
                    interpretations.get(i).unwrap().push(lhs);
                    s = rhs;
                }
            }

            Self {
                interpretations,
                _phantom: Default::default(),
            }
        }
    };
}

impl<'a, B: Backend, const STRIDE: usize> InterleavedBufferView<'a, B, STRIDE> {
    interpret!(
        (source: &'a DynamicMemoryBlock<B>) {
            use as_slice
            and split_array_ref
        }
    );

    pub fn get(&self, index: usize) -> Option<&Vec<&'a [u8; STRIDE]>> {
        self.interpretations.get(index)
    }
}

/// Takes an interleaved slice and locks its length, allowing concurrent writes to non-aliasing slices
pub struct InterleavedBufferViewMut<'a, B, const STRIDE: usize>
where
    B: Backend,
{
    interpretations: Vec<Vec<&'a mut [u8; STRIDE]>>,
    _phantom: PhantomData<B>,
}

impl<'a, B: Backend, const STRIDE: usize> InterleavedBufferViewMut<'a, B, STRIDE> {
    interpret!(
        (source: &'a mut DynamicMemoryBlock<B>) {
            use as_slice_mut
            and split_array_mut
        }
    );

    pub fn get(&self, index: usize) -> Option<&Vec<&'a mut [u8; STRIDE]>> {
        self.interpretations.get(index)
    }
}

pub struct InterleavedBuffer<B, const STRIDE: usize>
where
    B: Backend,
{
    buffer: DynamicMemoryBlock<B>,
    count: usize,
}

impl<B, const STRIDE: usize> InterleavedBuffer<B, STRIDE>
where
    B: Backend,
{
    /// Takes a source buffer and duplicates it count times
    pub async fn new_interleaved_from(
        backend: Arc<B>,
        source: &mut DynamicMemoryBlock<B>,
        count: usize,
    ) -> Self {
        let len = source.len().await * count;
        let src = source.as_device().await;
        let mut buffer = DynamicMemoryBlock::new(backend.clone(), len, None);

        {
            let dest = buffer.as_device().await;
            backend
                .get_utils()
                .interleave::<STRIDE>(src, dest, count)
                .await;
        }

        Self { buffer, count }
    }

    /// Borrow this buffer as a view
    pub async fn view<S: ToRange<usize> + Send>(
        &self,
        range: S,
    ) -> InterleavedBufferView<B, STRIDE> {
        InterleavedBufferView::interpret(&self.buffer, self.count, range).await
    }

    /// Borrow this buffer mutable as a mutable view
    pub async fn view_mut<S: ToRange<usize> + Send>(
        &mut self,
        range: S,
    ) -> InterleavedBufferViewMut<B, STRIDE> {
        InterleavedBufferViewMut::interpret(&mut self.buffer, self.count, range).await
    }
}
