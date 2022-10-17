use crate::fenwick::FenwickTree;
use crate::impl_concrete_ptr;
use crate::instance::memory::abstr::AbstractMemoryPtr;
use crate::memory::interleaved::{
    InterleavedBuffer, InterleavedBufferView, InterleavedBufferViewMut,
};
use crate::Backend;

const STRIDE: usize = 4;

pub struct MemoryInstanceSet<B>
where
    B: Backend,
{
    data: InterleavedBuffer<B, STRIDE>,
    lengths: FenwickTree,
    id: usize,
}

impl<B: Backend> MemoryInstanceSet<B> {
    pub async fn view<T>(&self) -> MemoryInstanceView<B> {
        MemoryInstanceView {
            id: self.id,
            view: self.data.view().await,
            lengths: self.lengths.clone(),
        }
    }

    pub async fn view_mut<T>(&mut self) -> MemoryInstanceViewMut<B> {
        MemoryInstanceViewMut {
            id: self.id,
            view: self.data.view_mut().await,
            lengths: self.lengths.clone(),
        }
    }
}

pub struct MemoryInstanceView<'a, B: Backend> {
    view: InterleavedBufferView<'a, B, STRIDE>,
    lengths: FenwickTree,
    id: usize,
}

impl<'a, B: Backend> MemoryInstanceView<'a, B> {
    pub fn get<'b: 'a, T>(&'b self, ptr: &MemoryPtr<B, T>) -> Option<&[&'a [u8; 4]]> {
        assert_eq!(self.id, ptr.src.id);

        let start = self.lengths.prefix_sum(ptr.src.ptr);
        let end = self.lengths.prefix_sum(ptr.src.ptr + 1);
        self.view.get(ptr.src.index).map(|v| v.slice(start, end))
    }
}

pub struct MemoryInstanceViewMut<'a, B: Backend> {
    view: InterleavedBufferViewMut<'a, B, STRIDE>,
    lengths: FenwickTree,
    id: usize,
}

impl<'a, B: Backend> MemoryInstanceViewMut<'a, B> {
    pub fn get<'b: 'a, T>(&'b self, ptr: &MemoryPtr<B, T>) -> Option<&[&'a mut [u8; 4]]> {
        assert_eq!(self.id, ptr.src.id);

        let start = self.lengths.prefix_sum(ptr.src.ptr);
        let end = self.lengths.prefix_sum(ptr.src.ptr + 1);
        self.view.get(ptr.src.index).map(|v| v.slice(start, end))
    }
}

impl_concrete_ptr!(
    pub struct MemoryPtr<B: Backend, T> {
        data...
    } with abstract AbstractMemoryPtr<B, T>;
);
