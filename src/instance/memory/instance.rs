use crate::capabilities::CapabilityStore;
use crate::impl_concrete_ptr;
use crate::instance::memory::builder::AbstractMemoryPtr;
use futures::future::join_all;
use wgpu_async::{async_device::OutOfMemoryError, async_queue::AsyncQueue};
use wgpu_lazybuffers::{MemorySystem, UnmappedLazyBuffer};
use wgpu_lazybuffers_interleaving::{
    Interleaveable, InterleavedBufferConfig, MappedInterleavedBuffer, UnmappedInterleavedBuffer,
};

const STRIDE: u64 = 16; // 4 * u32

#[derive(Clone)]
struct Meta {
    cap_set: CapabilityStore,
}

pub struct UnmappedMemoryInstanceSet {
    data: Vec<UnmappedInterleavedBuffer<STRIDE>>,
    meta: Meta,
}

impl UnmappedMemoryInstanceSet {
    pub(crate) async fn new(
        memory_system: &MemorySystem,
        queue: &AsyncQueue,
        sources: &Vec<UnmappedLazyBuffer>,
        count: usize,
        cap_set: CapabilityStore,
    ) -> Result<Self, OutOfMemoryError> {
        let memories = sources.iter().map(|source| {
            source.duplicate_interleave(
                memory_system,
                queue,
                &InterleavedBufferConfig {
                    repetitions: count,
                    usages: wgpu::BufferUsages::STORAGE,
                    locking_size: None,
                },
            )
        });

        let memories: Result<_, _> = join_all(memories).await.into_iter().collect();

        Ok(Self {
            data: memories?,
            meta: Meta { cap_set },
        })
    }
}

pub struct MappedMemoryInstanceSet {
    data: Vec<MappedInterleavedBuffer<STRIDE>>,
    meta: Meta,
}

/// A view of a memory for a specific wasm instance
pub struct MemoryView<'a> {
    buf: &'a MappedInterleavedBuffer<STRIDE>,
    index: usize,
}

impl<'a> MemoryView<'a> {
    pub async fn get(&self, index: usize) -> Option<&u8> {
        self.buf.get(self.index, index).await
    }
}

/// A mutable view of a memory for a specific wasm instance
pub struct MemoryViewMut<'a> {
    buf: &'a mut MappedInterleavedBuffer<STRIDE>,
    index: usize,
}

impl<'a> MemoryViewMut<'a> {
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

impl MappedMemoryInstanceSet {
    pub fn get<T>(&self, ptr: MemoryPtr<T>) -> MemoryView {
        return impl_get!(
            with self, ptr
            using get
            making MemoryView
        );
    }

    pub fn get_mut<T>(&mut self, ptr: MemoryPtr<T>) -> MemoryViewMut {
        return impl_get!(
            with self, ptr
            using get_mut
            making MemoryViewMut
        );
    }
}

impl_concrete_ptr!(
    pub struct MemoryPtr<T> {
        data...
    } with abstract AbstractMemoryPtr<T>;
);
