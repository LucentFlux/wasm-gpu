use crate::memory::DynamicMemoryBlock;
use crate::typed::ToRange;
use crate::{Backend, MainMemoryBlock};
use itertools::Itertools;
use std::ops::RangeBounds;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Acts like a slice, but deals with strides and offsets into the underlying buffer
pub struct InterleavedSlice<'a, 'b, B, const STRIDE: usize>
where
    B: Backend,
{
    /// While-ever a view exists, the buffer shouldn't be moved or resized
    memory: RwLockReadGuard<'a, &'b mut DynamicMemoryBlock<B>>,
    total: usize,  // Number of interleaved buffers
    len: usize,    // Number of values of length STRIDE that are in this 'slice'
    offset: usize, // Index (in bytes) of index 0 of this 'slice'
}

macro_rules! getter {
    (
        pub async fn $n1:tt(&self, ...) -> $(&[u8; $t1:tt])? $(&mut [u8; $t2:tt])? {
            ...
            self.memory.$m1:tt(bounds)
            ...
        }
    ) => {
        pub async fn $n1(&self, index: usize) -> $(&[u8; $t1])* $(&mut [u8; $t2])* {
            assert!(index < self.len);

            let start = self.offset + (index * self.total * $($t1)* $($t2)*);
            let end = start + $($t1)* $($t2)*;
            let bounds = start..end;

            let slice = self.memory.$m1(bounds).await;

            return slice.try_into().unwrap();
        }
    };
}

impl<'a, 'b, B, const STRIDE: usize> InterleavedSlice<'a, 'b, B, STRIDE>
where
    B: Backend,
{
    getter!(
        pub async fn get(&self, ...) -> &[u8; STRIDE] {
            ...
            self.memory.as_slice(bounds)
            ...
        }
    );
}

/// Acts like a slice, but deals with strides and offsets into the underlying buffer
pub struct InterleavedSliceMut<'a, 'b, B, const STRIDE: usize> {
    /// While-ever a view exists, the buffer shouldn't be moved or resized
    memory: RwLockWriteGuard<'a, &'b mut DynamicMemoryBlock<B>>,
    total: usize,  // Number of interleaved buffers
    len: usize,    // Number of values of length STRIDE that are in this 'slice'
    offset: usize, // Index (in bytes) of index 0 of this 'slice'
}

impl<'a, 'b, B, const STRIDE: usize> InterleavedSliceMut<'a, 'b, B, STRIDE>
where
    B: Backend,
{
    getter!(
        pub async fn get(&self, ...) -> &[u8; STRIDE] {
            ...
            self.memory.as_slice(bounds)
            ...
        }
    );

    getter!(
        pub async fn get_mut(&self, ...) -> &mut [u8; STRIDE] {
            ...
            self.memory.as_slice_mut(bounds)
            ...
        }
    );
}

/// STRIDE: The number of bytes to one value
pub struct InterleavedBufferInterpreter<'a, B, const STRIDE: usize>
where
    B: Backend,
{
    backing: Arc<RwLock<&'a mut DynamicMemoryBlock<B>>>, // Shared by all interleaved buffers of a set
    index: usize, // ASSERT: Unique over each InterleavedDynamicBufferInternal
    total: usize, // The total number of interleaved buffers pointing to the backing pointer
}

impl<'a, B, const STRIDE: usize> InterleavedBufferInterpreter<'a, B, STRIDE>
where
    B: Backend,
{
    /// Takes a memory block and interprets it as an interleaved buffer
    ///
    /// # Panics
    /// Count cannot be 0
    pub fn interpret(
        source: &'a mut DynamicMemoryBlock<B>,
        count: usize,
    ) -> Vec<InterleavedBufferInterpreter<'a, B, STRIDE>> {
        assert!(count > 0, "interleaved count cannot be zero");

        let backing = Arc::new(RwLock::new(source));

        (0..count)
            .map(|index| InterleavedBufferInterpreter {
                backing: backing.clone(),
                index,
                total: count,
            })
            .collect_vec()
    }

    /// The number of items (blocks of bytes of length STRIDE) that are in each interleaved buffer
    pub async fn len(&self) -> usize {
        return self.backing.read().unwrap().expect("buffer lost").len() / self.total;
    }

    /// Get a view into the slice given, for this portion of the interleaved buffer
    pub async fn get<S: RangeBounds<usize> + Send>(
        &self,
        bounds: S,
    ) -> InterleavedSlice<B, STRIDE> {
        let range = bounds.half_open();

        return InterleavedSlice {
            memory: self.backing.read().unwrap(),
            total: self.total,
            len: range.len(),
            offset: self.index + self.total * range.start,
        };
    }

    /// Get a mutable view into the slice given, for this portion of the interleaved buffer
    ///
    /// Note: We know slices can't alias - for host function performance, unsafe rust could be written for concurrent mutable access
    pub async fn get_mut<S: RangeBounds<usize> + Send>(
        &mut self,
        bounds: S,
    ) -> InterleavedSliceMut<B, STRIDE> {
        let range = bounds.half_open();

        return InterleavedSliceMut {
            memory: self.backing.write().unwrap(),
            total: self.total,
            len: range.len(),
            offset: self.index + self.total * range.start,
        };
    }
}

pub struct InterleavedBufferView<'a, B, const STRIDE: usize>
where
    B: Backend,
{
    interpretations: Vec<InterleavedBufferInterpreter<'a, B, STRIDE>>,
}

pub struct InterleavedBuffer<B, const STRIDE: usize>
where
    B: Backend,
{
    // Is none when the buffer is taken by a view. Should always be Some at function entry
    buffer: DynamicMemoryBlock<B>,
    total: usize,
}

impl<B, const STRIDE: usize> InterleavedBuffer<B, STRIDE>
where
    B: Backend,
{
    /// Takes a source buffer and duplicates it count times
    pub fn new_interleaved_from(source: &DynamicMemoryBlock<B>, count: usize) {}

    /// Borrow this buffer mutably as a view
    pub fn view(&mut self) -> InterleavedBufferView<B, STRIDE> {
        InterleavedBufferView {
            interpretations: InterleavedBufferInterpreter::interpret(&mut self.buffer, self.total),
        }
    }
}
