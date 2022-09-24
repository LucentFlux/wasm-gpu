use crate::Backend;
use wasmtime_environ::{MemoryPlan, TablePlan, WasmFuncType, WasmType};

/// Since all stores in a store set are instantiated the same from a builder,
/// this pointer actually points to a collection of locations,
/// i.e. all locations that correspond to the same logical WASM location inside any of the
/// stores created by a StoreSet with the ID held by this ptr.
///
/// Pointers should be created by a `StoreSetBuilder`, and then queried by a `Store`
pub trait StorePtr {
    fn get_store_id(&self) -> usize;
    fn get_ptr(&self) -> usize;
}

#[macro_export]
macro_rules! impl_ptr {
    (pub struct $name:ident <B, T> {... $($e_vis:vis $e_ident:ident : $e_type:ty ,)*}) => {
        pub struct $name<B, T>
        where
            B: Backend,
        {
            // Only make sense in the context of a specific store
            ptr: usize,
            store_id: usize,

            $($e_vis $e_ident : $e_type ,)*

            _phantom_data: PhantomData<(B, T)>,
        }

        impl<B, T> $name<B, T>
        where
            B: Backend,
        {
            pub fn new(ptr: usize, store_id: usize $(, $e_ident : $e_type)*) -> Self {
                Self {
                    ptr,
                    store_id,
                    $($e_ident ,)*
                    _phantom_data: Default::default(),
                }
            }
        }

        impl<B, T> Clone for $name<B, T>
        where
            B: Backend,
        {
            fn clone(&self) -> Self {
                Self {
                    ptr: self.ptr.clone(),
                    store_id: self.store_id.clone(),
                    $($e_ident : self.$e_ident.clone() ,)*
                    _phantom_data: Default::default(),
                }
            }
        }

        impl<B, T> StorePtr for $name<B, T>
        where
            B: Backend,
        {
            fn get_store_id(&self) -> usize {
                self.store_id
            }
            fn get_ptr(&self) -> usize {
                self.ptr
            }
        }

        impl<B, T> Hash for $name<B, T>
        where
            B: Backend,
        {
            fn hash<H: Hasher>(&self, state: &mut H) {
                state.write_usize(self.store_id);
                state.write_usize(self.ptr);
            }
        }

        impl<B, T> PartialEq<Self> for $name<B, T>
        where
            B: Backend,
        {
            fn eq(&self, other: &Self) -> bool {
                self.store_id == other.store_id && self.ptr == other.ptr
            }
        }

        impl<B, T> Eq for $name<B, T> where B: Backend {}

        impl<B, T> PartialOrd<Self> for $name<B, T>
        where
            B: Backend,
        {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl<B, T> Ord for $name<B, T>
        where
            B: Backend,
        {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                match self.store_id.cmp(&other.store_id) {
                    std::cmp::Ordering::Equal => self.ptr.cmp(&other.ptr),
                    v => v,
                }
            }
        }
    };
}

// Note: Some pointers carry type information. This information is treated as immutable
impl_ptr!(
    pub struct FuncPtr<B, T> {
        ...
        // Copied from Func
        ty: WasmFuncType,
    }
);

impl_ptr!(
    pub struct MemoryPtr<B, T> {
        ...
        // Copied from Memory
        minimum: u64,
        maximum: Option<u64>,
    }
);

impl_ptr!(
    pub struct TablePtr<B, T> {
        ...
        // Copied from Table
        wasm_ty: WasmType,
        minimum: u32,
        maximum: Option<u32>,
    }
);

impl<B, T> FuncPtr<B, T>
where
    B: Backend,
{
    pub fn params(&self) -> &[WasmType] {
        return self.ty.params();
    }

    pub fn results(&self) -> &[WasmType] {
        return self.ty.returns();
    }

    pub fn is_type(&self, ty: &WasmFuncType) -> bool {
        return self.ty.eq(ty);
    }
}

fn limits_match<V: Ord>(n1: V, m1: Option<V>, n2: V, m2: Option<V>) -> bool {
    if n1 > n2 {
        return false;
    }
    return match (m1, m2) {
        (None, None) => true,
        (Some(m1), Some(m2)) => (m1 >= m2),
        (_, _) => false,
    };
}

impl<B, T> MemoryPtr<B, T>
where
    B: Backend,
{
    pub fn is_memory_type(&self, ty: &wasmtime_environ::Memory) -> bool {
        // Imagine: Can this be used as a memory of type ty
        limits_match(self.minimum, self.maximum, ty.minimum, ty.maximum)
    }

    pub fn is_type(&self, ty: &MemoryPlan) -> bool {
        self.is_memory_type(&ty.memory)
    }
}

impl<B, T> TablePtr<B, T>
where
    B: Backend,
{
    pub fn is_table_type(&self, ty: &wasmtime_environ::Table) -> bool {
        return self.wasm_ty.eq(&ty.wasm_ty)
            && limits_match(self.minimum, self.maximum, ty.minimum, ty.maximum);
    }

    pub fn is_type(&self, ty: &TablePlan) -> bool {
        self.is_table_type(&ty.table)
    }
}
