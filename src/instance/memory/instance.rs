use crate::capabilities::CapabilityStore;
use crate::fenwick::FenwickTree;
use crate::impl_concrete_ptr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use futures::future::join_all;
use lf_hal::backend::Backend;
use lf_hal::memory::interleaved::{DeviceInterleavedBuffer, HostInterleavedBuffer};
use lf_hal::memory::{DeviceMemoryBlock, MemoryBlock};

const STRIDE: usize = 4; // 4 * u32

#[derive(Clone)]
struct Meta {
    lengths: FenwickTree,
    cap_set: CapabilityStore,
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
        sources: &Vec<B::DeviceMemoryBlock>,
        count: usize,
        cap_set: CapabilityStore,
    ) -> Self {
        let memories = sources.iter().map(|source: &B::DeviceMemoryBlock| async {
            (source.len(), source.interleave(count).await)
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
            meta: Meta { lengths, cap_set },
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
    pub async fn get(&self, index: usize) -> Option<&u8> {
        self.buf.get(self.index, index).await
    }
}

/// A mutable view of a memory for a specific wasm instance
pub struct MemoryViewMut<'a, B: Backend> {
    buf: &'a mut HostInterleavedBuffer<B, STRIDE>,
    index: usize,
}

impl<'a, B: Backend> MemoryViewMut<'a, B> {
    pub async fn get_mut(&'a mut self, index: usize) -> Option<&'a mut u8> {
        self.buf.get_mut(self.index, index).await
    }
}

macro_rules! impl_get {
    (with $self:ident, $ptr:ident using $get:ident making $MemoryView:ident) => {{
        assert!(
            $self.meta.cap_set.check(&$ptr.src.cap),
            "memory pointer was not valid for this instance"
        );
        let buf = $self
            .data
            .$get($ptr.src.ptr)
            .expect("memory pointer was valid but malformed - this is a bug");
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
