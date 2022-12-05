use crate::fenwick::FenwickTree;
use crate::impl_concrete_ptr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use futures::future::join_all;
use itertools::Itertools;
use lib_hal::backend::Backend;
use lib_hal::memory::interleaved::{
    DeviceInterleavedBuffer, HostInterleavedBuffer, InterleavedBufferView, InterleavedBufferViewMut,
};
use lib_hal::memory::MemoryBlock;
use std::sync::Arc;

const STRIDE: usize = 4; // 4 * u32

#[derive(Clone)]
struct Meta {
    lengths: FenwickTree,
    id: usize,
}

pub struct UnmappedMemoryInstanceSet<B>
where
    B: Backend,
{
    data: Vec<DeviceInterleavedBuffer<B, STRIDE>>,
    meta: Meta,
}

impl<B> UnmappedMemoryInstanceSet<B>
where
    B: Backend,
{
    pub(crate) async fn new(
        backend: Arc<B>,
        sources: &Vec<B::DeviceMemoryBlock>,
        count: usize,
        id: usize,
    ) -> Self {
        let memories = sources.iter().map(|source: &B::DeviceMemoryBlock| async {
            (
                source.len(),
                DeviceInterleavedBuffer::new_interleaved_from(backend.clone(), source, count).await,
            )
        });
        let memory_and_infos = join_all(memories).await;
        let lengths = memory_and_infos.iter().map(|(len, _)| *len);
        let lengths = FenwickTree::new(lengths);
        let data: Vec<_> = memory_and_infos
            .into_iter()
            .map(|(_, memory)| memory)
            .collect();
        Self {
            data,
            meta: Meta { lengths, id },
        }
    }
}

pub struct MappedMemoryInstanceSet<B>
where
    B: Backend,
{
    data: Vec<HostInterleavedBuffer<B, STRIDE>>,
    meta: Meta,
}

/// A view of a memory for a specific wasm instance
pub struct MemoryView<'a, B: Backend> {
    buf: &'a HostInterleavedBuffer<B, STRIDE>,
    index: usize,
}

impl<'a, B: Backend> MemoryView<'a, B> {
    pub async fn get(&self, index: usize) -> u8 {
        let chunk = index / STRIDE;
        let offset = index % STRIDE;
        let view: InterleavedBufferView = self.buf.get(chunk..=chunk).await;

        let vec = view
            .get(self.index)
            .expect("memory index offset out of bounds")
            .collect_vec();

        assert_eq!(vec.len(), STRIDE);

        return *vec[offset];
    }
}

/// A mutable view of a memory for a specific wasm instance
pub struct MemoryViewMut<'a, B: Backend> {
    buf: &'a mut HostInterleavedBuffer<B, STRIDE>,
    index: usize,
}

impl<'a, B: Backend> MemoryViewMut<'a, B> {
    pub async fn get_mut(&'a mut self, index: usize) -> &'a mut u8 {
        let chunk = index / STRIDE;
        let offset = index % STRIDE;
        let view: InterleavedBufferViewMut = self.buf.get_mut(chunk..=chunk).await;

        return view
            .take(self.index)
            .expect("memory index offset out of bounds")
            .skip(offset)
            .next()
            .expect(
                format!(
                    "chunk of size {} did not have an index {} element",
                    STRIDE, offset
                )
                .as_str(),
            );
    }
}

macro_rules! impl_get {
    (with $self:ident, $ptr:ident using $get:ident making $MemoryView:ident) => {{
        assert_eq!(
            $ptr.src.id, $self.meta.id,
            "memory pointer does not belong to this memory instance set"
        );
        let buf = $self
            .data
            .$get($ptr.src.ptr)
            .expect("memory pointer was valid but malformed");
        $MemoryView {
            buf,
            index: $ptr.index,
        }
    }};
}

impl<B: Backend> MappedMemoryInstanceSet<B> {
    pub fn get<T>(&self, ptr: MemoryPtr<B, T>) -> MemoryView<B> {
        return impl_get!(
            with self, ptr
            using get
            making MemoryView
        );
    }

    pub fn get_mut<T>(&mut self, ptr: MemoryPtr<B, T>) -> MemoryViewMut<B> {
        return impl_get!(
            with self, ptr
            using get_mut
            making MemoryViewMut
        );
    }
}

impl_concrete_ptr!(
    pub struct MemoryPtr<B: Backend, T> {
        data...
    } with abstract AbstractMemoryPtr<B, T>;
);
