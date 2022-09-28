use crate::Backend;
use paste::paste;
use wasmparser::{FuncType, MemoryType, TableType};
use wasmtime_environ::{MemoryPlan, TablePlan, WasmFuncType, WasmType};

#[macro_export]
macro_rules! impl_ptr {
    (
        pub struct $name:ident <B, T> {
            ...
            // Some pointers carry type information. This information is treated as immutable
            $($e_vis:vis $e_ident:ident : $e_type:ty),* $(,)?
        }

        $(
            impl<B, T> $impl_name:ident <B, T> {
                $($impl_code:tt)*
            }
        )?
    ) => {
        paste! {
            #[derive(Debug)]
            #[doc="Since all stores in a concrete store set are instantiated from a builder, \
            this pointer actually points to a collection of locations, \
            i.e. all locations that correspond to the same logical WASM location \
            inside any of the stores created by a StoreSet with the ID held by this ptr."]
            pub struct [< Abstract $name >]<B, T>
            where
                B: Backend,
            {
                // Only make sense in the context of a specific abstract store
                ptr: usize,
                store_id: usize,

                $($e_vis $e_ident : $e_type ,)*

                _phantom_data: PhantomData<(B, T)>,
            }

            #[derive(Debug)]
            pub struct $name<B, T>
            where
                B: Backend,
            {
                // Only make sense in the context of a specific concrete store
                ptr: usize,
                abstract_store_id: usize,
                concrete_store_id: usize,

                $($e_vis $e_ident : $e_type ,)*

                _phantom_data: PhantomData<(B, T)>,
            }

            impl<B, T> [< Abstract $name >]<B, T>
            where
                B: Backend,
            {
                fn new(ptr: usize, store_id: usize $(, $e_ident : $e_type)*) -> Self {
                    Self {
                        ptr,
                        store_id,
                        $($e_ident ,)*
                        _phantom_data: Default::default(),
                    }
                }

                pub fn concrete(&self, concrete_id: usize) -> $name<B, T> {
                    $name {
                        ptr: self.ptr,
                        abstract_store_id: self.store_id,
                        concrete_store_id: self.concrete_id,
                        $($e_ident ,)*
                        _phantom_data: Default::default(),
                    }
                }

                $(
                    $($impl_code)*
                )*
            }

            impl<B, T> $name<B, T>
            where
                B: Backend,
            {
                $(
                    $($impl_code)*
                )*
            }

            impl<B, T> Clone for [< Abstract $name >]<B, T>
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

            impl<B, T> Clone for $name<B, T>
            where
                B: Backend,
            {
                fn clone(&self) -> Self {
                    Self {
                        ptr: self.ptr.clone(),
                        abstract_store_id: self.abstract_store_id.clone(),
                        concrete_store_id: self.concrete_store_id.clone(),
                        $($e_ident : self.$e_ident.clone() ,)*
                        _phantom_data: Default::default(),
                    }
                }
            }

            impl<B, T> Hash for [< Abstract $name >]<B, T>
            where
                B: Backend,
            {
                fn hash<H: Hasher>(&self, state: &mut H) {
                    state.write_usize(self.store_id);
                    state.write_usize(self.ptr);
                }
            }

            impl<B, T> Hash for $name<B, T>
            where
                B: Backend,
            {
                fn hash<H: Hasher>(&self, state: &mut H) {
                    state.write_usize(self.concrete_store_id);
                    state.write_usize(self.abstract_store_id);
                    state.write_usize(self.ptr);
                }
            }

            impl<B, T> PartialEq<Self> for [< Abstract $name >]<B, T>
            where
                B: Backend,
            {
                fn eq(&self, other: &Self) -> bool {
                    self.store_id == other.store_id && self.ptr == other.ptr
                }
            }

            impl<B, T> PartialEq<Self> for $name<B, T>
            where
                B: Backend,
            {
                fn eq(&self, other: &Self) -> bool {
                    self.concrete_store_id == other.concrete_store_id && self.abstract_store_id == other.abstract_store_id && self.ptr == other.ptr
                }
            }

            impl<B, T> Eq for [< Abstract $name >]<B, T> where B: Backend {}

            impl<B, T> Eq for $name<B, T> where B: Backend {}

            impl<B, T> PartialOrd<Self> for [< Abstract $name >]<B, T>
            where
                B: Backend,
            {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }

            impl<B, T> PartialOrd<Self> for $name<B, T>
            where
                B: Backend,
            {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }

            impl<B, T> Ord for [< Abstract $name >]<B, T>
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

            impl<B, T> Ord for $name<B, T>
            where
                B: Backend,
            {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    match self.abstract_store_id.cmp(&other.abstract_store_id) {
                        std::cmp::Ordering::Equal => match self.concrete_store_id.cmp(&other.concrete_store_id) {
                            std::cmp::Ordering::Equal => self.ptr.cmp(&other.ptr),
                            v => v,
                        }
                        v => v,
                    }
                }
            }
        }
    };
}
