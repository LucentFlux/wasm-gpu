use crate::fenwick::FenwickTree;
use crate::impl_concrete_ptr;
use crate::instance::memory::abstr::AbstractMemoryPtr;
use crate::memory::interleaved::{
    DeviceInterleavedBuffer, HostInterleavedBuffer, InterleavedBufferView,
};
use crate::Backend;

const STRIDE: usize = 4; // 4 * u32

#[derive(Clone)]
struct Meta {
    lengths: FenwickTree,
    id: usize,
}

pub struct DeviceMemoryInstanceSet<B>
where
    B: Backend,
{
    data: Vec<DeviceInterleavedBuffer<B, STRIDE>>,
    meta: Meta,
}

pub struct HostMemoryInstanceSet<B>
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

        match view
            .get(self.index)
            .expect("memory index out of bounds")
            .as_slice()
        {
            [v] => v[offset],
            _ => panic!("failed to get single chunk"),
        }
    }
}

/// A mutable view of a memory for a specific wasm instance
pub struct MemoryViewMut<'a, B: Backend> {
    buf: &'a mut HostInterleavedBuffer<B, STRIDE>,
    index: usize,
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

impl<B: Backend> HostMemoryInstanceSet<B> {
    pub fn get<T>(&self, ptr: MemoryPtr<B, T>) -> MemoryView<B> {
        return impl_get!(
            with self, ptr
            using get
            making MemoryView
        );
    }

    pub fn get_mut<T>(&self, ptr: MemoryPtr<B, T>) -> MemoryViewMut<B> {
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
