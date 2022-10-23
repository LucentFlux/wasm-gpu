pub(crate) trait AbstractPtr {
    type ConcretePtr: ConcretePtr;
    fn concrete(&self, index: usize) -> Self::ConcretePtr;
}

pub(crate) trait ConcretePtr {
    type AbstractPtr: AbstractPtr;
    fn from(abst: Self::AbstractPtr, index: usize) -> Self;
    fn abstr(&self) -> &Self::AbstractPtr;
}

#[macro_export]
macro_rules! impl_immutable_ptr {
    (
        pub struct $name:ident $(<$($lt:tt $( : $clt:tt $(+ $dlt:tt )* )?),* $(,)?>)? {
            $v:vis $_:ident...
            // Some pointers carry type information. This information is treated as immutable
            $($e_vis:vis $e_ident:ident : $e_type:ty),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        #[doc="Since all stores in a concrete store_set set are instantiated from a builder, \
        this pointer actually points to a collection of locations, \
        i.e. all locations that correspond to the same logical WASM location \
        inside any of the stores created by a StoreSet with the ID held by this ptr."]
        pub struct $name $(<$($lt $(: $clt $(+ $dlt)*)*),*>)*
        {
            #[doc="Point within the data as seen by WASM"]
            $v ptr: usize,
            $v id: usize,

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
        where $( $e_type : Clone,)*
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
        where $( $e_type : std::hash::Hash,)*
        {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                state.write_usize(self.id);
                state.write_usize(self.ptr);
                $(
                    self.$e_ident.hash(&mut state);
                )*
            }
        }

        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialEq<Self> for $name $(<$($lt),*>)*
        where $( $e_type : PartialEq<$e_type>,)*
        {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id && self.ptr == other.ptr
                $( && self.$e_ident.eq(other.$e_ident))*
            }
        }
        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Eq for $name$(<$($lt),*>)*
        where $( $e_type : Eq,)* {}

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialOrd<Self> for $name$(<$($lt),*>)*
        {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                let success = match self.id.cmp(&other.id) {
                    std::cmp::Ordering::Equal => match self.ptr.cmp(&other.ptr) {
                        std::cmp::Ordering::Equal => return None,
                        v => v,
                    }
                    v => v,
                };

                return Some(success)
            }
        }

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Ord for $name$(<$($lt),*>)*
        where $( $e_type : Ord,)*
        {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                let c = self.id.cmp(&other.id);
                if c != std::cmp::Ordering::Equal {
                    return c;
                }

                let c = self.ptr.cmp(&other.ptr);
                if c != std::cmp::Ordering::Equal {
                    return c;
                }

                $(
                    let c = self.$e_ident.cmp(&other.$e_ident);
                    if c != std::cmp::Ordering::Equal {
                        return c;
                    }
                )*

                return c;
            }
        }
    };
}

#[macro_export]
macro_rules! impl_abstract_ptr {
    (
        pub struct $name:ident $(<$($lt:tt $( : $clt:tt $(+ $dlt:tt )* )?),* $(,)?>)? {
            $v:vis $_:ident...
            // Some pointers carry type information. This information is treated as immutable
            $($e_vis:vis $e_ident:ident : $e_type:ty),* $(,)?
        } with concrete $concrete:ident $(<$($cct:tt),* $(,)?>)?;
    ) => {
        crate::impl_immutable_ptr!(
            pub struct $name $(<$($lt $( : $clt $(+ $dlt )* )*),* >)* {
                $v data...
                $($e_vis $e_ident : $e_type),*
            }
        );

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* crate::instance::ptrs::AbstractPtr for $name $(<$($lt),*>)*
        {
            type ConcretePtr = $concrete $(<$($cct),*>)*;

            fn concrete(&self,
                index: usize,
            ) -> Self::ConcretePtr
            {
                let v = self.clone();
                <Self::ConcretePtr as crate::instance::ptrs::ConcretePtr>::from (
                    v,
                    index,
                )
            }
        }
    }
}

#[macro_export]
macro_rules! impl_concrete_ptr {
    (
        pub struct $name:ident $(<$($lt:tt $( : $clt:tt $(+ $dlt:tt )* )?),* $(,)?>)? {
            $v:vis $_:ident...
            $($e_vis:vis $e_ident:ident : $e_type:ty),* $(,)?
        } with abstract $abst:ident $(<$($at:tt),* $(,)?>)?;
    ) => {
        #[derive(Debug)]
        pub struct $name $(<$($lt $(: $clt $(+ $dlt)*)*),*>)*
        {
            #[doc="The abstract version of this pointer, pointing to the same place in every instance"]
            $v src: <Self as crate::instance::ptrs::ConcretePtr>::AbstractPtr,

            #[doc="Index of which WASM instance this is a pointer for"]
            $v index: usize,

            $($e_vis $e_ident : $e_type),*

            _phantom_data: std::marker::PhantomData<($($($lt ,)*)*)>,
        }


        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Clone for $name $(<$($lt),*>)*
        where <Self as crate::instance::ptrs::ConcretePtr>::AbstractPtr : Clone, $( $e_type : Clone,)*
        {
            fn clone(&self) -> Self {
                Self {
                    src: self.src.clone(),
                    index: self.index.clone(),
                    $($e_ident : self.$e_ident.clone() ,)*
                    _phantom_data: Default::default(),
                }
            }
        }

        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* std::hash::Hash for $name $(<$($lt),*>)*
        where <Self as crate::instance::ptrs::ConcretePtr>::AbstractPtr : std::hash::Hash, $( $e_type : std::hash::Hash,)*
        {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.src.hash(&mut state);
                state.write_usize(self.index);
                $(
                    self.$e_ident.hash(&mut state);
                )*
            }
        }

        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialEq<Self> for $name $(<$($lt),*>)*
        where <Self as crate::instance::ptrs::ConcretePtr>::AbstractPtr : PartialEq<<Self as crate::instance::ptrs::ConcretePtr>::AbstractPtr>,
            $( $e_type : PartialEq<$e_type>,)*
        {
            fn eq(&self, other: &Self) -> bool {
                self.src.eq(other.src) && self.index == other.index
                $( && self.$e_ident.eq(other.$e_ident))*
            }
        }
        impl $(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Eq for $name$(<$($lt),*>)*
        where <Self as crate::instance::ptrs::ConcretePtr>::AbstractPtr : Eq, $( $e_type : Eq,)* {}

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* PartialOrd<Self> for $name$(<$($lt),*>)*
        {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                let success = match self.src.partial_cmp(&other.src) {
                    None => return None,
                    Some(std::cmp::Ordering::Equal) => match self.index.cmp(&other.index) {
                        std::cmp::Ordering::Equal => return None,
                        v => v,
                    }
                    Some(v) => v,
                };

                return Some(success)
            }
        }

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* Ord for $name$(<$($lt),*>)*
        where <Self as crate::instance::ptrs::ConcretePtr>::AbstractPtr : Ord, $( $e_type : Ord,)*
        {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                let c = self.src.cmp(&other.src);
                if c != std::cmp::Ordering::Equal {
                    return c;
                }

                let c = self.index.cmp(&other.index);
                if c != std::cmp::Ordering::Equal {
                    return c;
                }

                $(
                    let c = self.$e_ident.cmp(&other.$e_ident);
                    if c != std::cmp::Ordering::Equal {
                        return c;
                    }
                )*

                return c;
            }
        }

        impl$(<$($lt $(: $clt $(+ $dlt)*)*),*>)* crate::instance::ptrs::ConcretePtr for $name $(<$($lt),*>)*
        {
            type AbstractPtr = $abst $(<$($at),*>)*;

            fn from(src: Self::AbstractPtr, index: usize) -> Self
            {
                Self {
                    src,
                    index,
                    _phantom_data: Default::default(),
                }
            }

            fn abstr(&self) -> &Self::AbstractPtr
            {
                &self.src
            }
        }
    }
}
