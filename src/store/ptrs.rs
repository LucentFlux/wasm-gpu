pub(crate) trait AbstractPtr {
    type ConcretePtr;
    fn concrete(&self, concrete_id: usize) -> Self::ConcretePtr;
}

pub(crate) trait ConcretePtr {
    type AbstractPtr;
    fn as_abstract(&self) -> Self::AbstractPtr;
}

#[macro_export]
macro_rules! impl_ptr {
    (
        pub struct $name:ident $(<$($lt:tt $( : $clt:tt $(+ $dlt:tt )* )?),* $(,)?>)? {
            ...
            // Some pointers carry type information. This information is treated as immutable
            $($e_vis:vis $e_ident:ident : $e_type:ty),* $(,)?
        }

        $(
            impl$(<$($it:tt $( : $cit:tt $(+ $dit:tt )* )?),* $(,)?>)? $impl_name:ident $(<$($it2:tt),* $(,)?>)? {
                $($impl_code:tt)*
            }
        )*
    ) => {
        paste::paste! {
            #[derive(Debug)]
            #[doc="Since all stores in a concrete store set are instantiated from a builder, \
            this pointer actually points to a collection of locations, \
            i.e. all locations that correspond to the same logical WASM location \
            inside any of the stores created by a StoreSet with the ID held by this ptr."]
            pub struct [< Abstract $name >] $(<$($lt $(: $clt $(+ $dlt)*)*),*>)*
            {
                // Only make sense in the context of a specific abstr store
                ptr: usize,
                store_id: usize,

                $($e_vis $e_ident : $e_type ,)*

                _phantom_data: std::marker::PhantomData<($($($lt ,)*)*)>,
            }

            #[derive(Debug)]
            pub struct $name $(<$($lt $(: $clt $(+ $dlt)*)*),*>)*
            {
                // Only make sense in the context of a specific concrete store
                ptr: usize,
                pub(crate) abstract_store_id: usize,
                pub(crate) concrete_store_id: usize,

                $($e_vis $e_ident : $e_type ,)*

                _phantom_data: std::marker::PhantomData<($($($lt ,)*)*)>,
            }

            $(
                impl $(<$($it $(: $cit $(+ $dit)*)*),*>)* [< Abstract $impl_name >] $(<$($it2),*>)*
                {
                    $($impl_code)*
                }
            )*

            impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* [< Abstract $name >] $(<$($lt),*>)*
            {
                fn new(ptr: usize, store_id: usize $(, $e_ident : $e_type)*) -> Self {
                    Self {
                        ptr,
                        store_id,
                        $($e_ident ,)*
                        _phantom_data: Default::default(),
                    }
                }
            }

            impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* crate::store::ptrs::AbstractPtr for [< Abstract $name >] $(<$($lt),*>)*
            {
                type ConcretePtr = $name $(<$($lt),*>)*;

                fn concrete(&self, concrete_id: usize) -> Self::ConcretePtr
                {
                    let v = self.clone();
                    $name {
                        ptr: v.ptr,
                        abstract_store_id: v.store_id,
                        concrete_store_id: concrete_id,
                        $($e_ident : v.$e_ident,)*
                        _phantom_data: Default::default(),
                    }
                }
            }

            impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* crate::store::ptrs::ConcretePtr for $name $(<$($lt),*>)*
            {
                type AbstractPtr = [< Abstract $name >] $(<$($lt),*>)*;

                fn as_abstract(&self) -> Self::AbstractPtr
                {
                    let v = self.clone();
                    $name {
                        ptr: v.ptr,
                        store_id: v.abstract_store_id,
                        $($e_ident : v.$e_ident,)*
                        _phantom_data: Default::default(),
                    }
                }
            }

            $(
                impl$(<$($it $(: $cit $(+ $dit)*)*),*>)* $impl_name $(<$($it2),*>)*
                {
                    $($impl_code)*
                }
            )*

            impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Clone for [< Abstract $name >] $(<$($lt),*>)*
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

            impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Clone for $name $(<$($lt),*>)*
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

            impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* std::hash::Hash for [< Abstract $name >] $(<$($lt),*>)*
            {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    state.write_usize(self.store_id);
                    state.write_usize(self.ptr);
                }
            }

            impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* std::hash::Hash for $name $(<$($lt),*>)*
            {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    state.write_usize(self.concrete_store_id);
                    state.write_usize(self.abstract_store_id);
                    state.write_usize(self.ptr);
                }
            }

            impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialEq<Self> for [< Abstract $name >] $(<$($lt),*>)*
            {
                fn eq(&self, other: &Self) -> bool {
                    self.store_id == other.store_id && self.ptr == other.ptr
                }
            }

            impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialEq<Self> for $name $(<$($lt),*>)*
            {
                fn eq(&self, other: &Self) -> bool {
                    self.concrete_store_id == other.concrete_store_id && self.abstract_store_id == other.abstract_store_id && self.ptr == other.ptr
                }
            }

            impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Eq for [< Abstract $name >]$(<$($lt),*>)* where B: Backend {}

            impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Eq for $name$(<$($lt),*>)* where B: Backend {}

            impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialOrd<Self> for [< Abstract $name >]$(<$($lt),*>)*
            {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }

            impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialOrd<Self> for $name$(<$($lt),*>)*
            {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }

            impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Ord for [< Abstract $name >]$(<$($lt),*>)*
            {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    match self.store_id.cmp(&other.store_id) {
                        std::cmp::Ordering::Equal => self.ptr.cmp(&other.ptr),
                        v => v,
                    }
                }
            }

            impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Ord for $name$(<$($lt),*>)*
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
