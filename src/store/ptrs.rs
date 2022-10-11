pub(crate) trait AbstractPtr {
    type ConcretePtr;
    fn concrete(&self, concrete_id: usize) -> Self::ConcretePtr;
}

pub(crate) trait ConcretePtr {
    type AbstractPtr;
    fn as_abstract(&self) -> Self::AbstractPtr;
}

#[macro_export]
macro_rules! impl_immutable_ptr {
    (
        pub struct $name:ident $(<$($lt:tt $( : $clt:tt $(+ $dlt:tt )* )?),* $(,)?>)? {
            ...
            // Some pointers carry type information. This information is treated as immutable
            $($e_vis:vis $e_ident:ident : $e_type:ty),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        #[doc="Since all stores in a concrete store set are instantiated from a builder, \
        this pointer actually points to a collection of locations, \
        i.e. all locations that correspond to the same logical WASM location \
        inside any of the stores created by a StoreSet with the ID held by this ptr."]
        pub struct $name $(<$($lt $(: $clt $(+ $dlt)*)*),*>)*
        {
            // Only make sense in the context of a specific object
            ptr: usize,
            id: usize,

            $($e_vis $e_ident : $e_type ,)*

            _phantom_data: std::marker::PhantomData<($($($lt ,)*)*)>,
        }

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* $name $(<$($lt),*>)*
        {
            fn new(ptr: usize, id: usize $(, $e_ident : $e_type)*) -> Self {
                Self {
                    ptr,
                    id,
                    $($e_ident ,)*
                    _phantom_data: Default::default(),
                }
            }
        }

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Clone for $name $(<$($lt),*>)*
        {
            fn clone(&self) -> Self {
                Self {
                    ptr: self.ptr.clone(),
                    id: self.id.clone(),
                    $($e_ident : self.$e_ident.clone() ,)*
                    _phantom_data: Default::default(),
                }
            }
        }

        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* std::hash::Hash for $name $(<$($lt),*>)*
        {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                state.write_usize(self.id);
                state.write_usize(self.ptr);
            }
        }

        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialEq<Self> for $name $(<$($lt),*>)*
        {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id && self.ptr == other.ptr
            }
        }
        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Eq for $name$(<$($lt),*>)* where B: Backend {}

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialOrd<Self> for $name$(<$($lt),*>)*
        {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Ord for $name$(<$($lt),*>)*
        {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                match self.id.cmp(&other.id) {
                    std::cmp::Ordering::Equal => self.ptr.cmp(&other.ptr),
                    v => v,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! impl_abstract_ptr {
    (
        pub struct $name:ident $(<$($lt:tt $( : $clt:tt $(+ $dlt:tt )* )?),* $(,)?>)? {
            ...
            // Some pointers carry type information. This information is treated as immutable
            $($e_vis:vis $e_ident:ident : $e_type:ty),* $(,)?
        } with concrete $concrete:ident $(<$($cct:tt),* $(,)?>)?;
    ) => {
        crate::impl_immutable_ptr!(
            pub struct $name $(<$($lt $( : $clt $(+ $dlt )* )*),* >)* {
                ...
                $($e_vis $e_ident : $e_type),*
            }
        );

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* crate::store::ptrs::AbstractPtr for $name $(<$($lt),*>)*
        {
            type ConcretePtr = $concrete $(<$($cct),*>)*;

            fn concrete(&self, concrete_id: usize) -> Self::ConcretePtr
            {
                let v = self.clone();
                $concrete::new (
                    v.ptr,
                    v.id,
                    concrete_id,
                    $(v.$e_ident,)*
                )
            }
        }
    }
}

#[macro_export]
macro_rules! impl_concrete_ptr {
    (
        pub struct $name:ident $(<$($lt:tt $( : $clt:tt $(+ $dlt:tt )* )?),* $(,)?>)? {
            ...
            // Some pointers carry type information. This information is treated as immutable
            $($e_vis:vis $e_ident:ident : $e_type:ty),* $(,)?
        } with abstract $abst:ident $(<$($at:tt),* $(,)?>)?;
    ) => {
        #[derive(Debug)]
        pub struct $name $(<$($lt $(: $clt $(+ $dlt)*)*),*>)*
        {
            // Only make sense in the context of a specific concrete store
            ptr: usize,
            abstract_id: usize,
            concrete_id: usize,

            $($e_vis $e_ident : $e_type ,)*

            _phantom_data: std::marker::PhantomData<($($($lt ,)*)*)>,
        }

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* $name $(<$($lt),*>)*
        {
            fn new(ptr: usize, abstract_id: usize, concrete_id: usize $(, $e_ident : $e_type)*) -> Self {
                Self {
                    ptr,
                    abstract_id,
                    concrete_id,
                    $($e_ident ,)*
                    _phantom_data: Default::default(),
                }
            }
        }

        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Clone for $name $(<$($lt),*>)*
        {
            fn clone(&self) -> Self {
                Self {
                    ptr: self.ptr.clone(),
                    abstract_id: self.abstract_id.clone(),
                    concrete_id: self.concrete_id.clone(),
                    $($e_ident : self.$e_ident.clone() ,)*
                    _phantom_data: Default::default(),
                }
            }
        }

        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* std::hash::Hash for $name $(<$($lt),*>)*
        {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                state.write_usize(self.concrete_id);
                state.write_usize(self.abstract_id);
                state.write_usize(self.ptr);
            }
        }

        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialEq<Self> for $name $(<$($lt),*>)*
        {
            fn eq(&self, other: &Self) -> bool {
                self.concrete_id == other.concrete_id && self.abstract_id == other.abstract_id && self.ptr == other.ptr
            }
        }
        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Eq for $name$(<$($lt),*>)* where B: Backend {}

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialOrd<Self> for $name$(<$($lt),*>)*
        {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Ord for $name$(<$($lt),*>)*
        {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                match self.abstract_id.cmp(&other.abstract_id) {
                    std::cmp::Ordering::Equal => match self.concrete_id.cmp(&other.concrete_id) {
                        std::cmp::Ordering::Equal => self.ptr.cmp(&other.ptr),
                        v => v,
                    }
                    v => v,
                }
            }
        }

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* crate::store::ptrs::ConcretePtr for $name $(<$($lt),*>)*
        {
            type AbstractPtr = $abst $(<$($at),*>)*;

            fn as_abstract(&self) -> Self::AbstractPtr
            {
                let v = self.clone();
                $abst::new(
                    v.ptr,
                    v.abstract_id,
                    $(v.$e_ident,)*
                )
            }
        }
    }
}
