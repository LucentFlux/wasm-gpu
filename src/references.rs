use std::ops::Deref;

macro_rules! impl_index {
    (pub struct $name:ident) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name (u32);

        impl $name {
            pub fn as_usize(&self) -> usize {
                usize::try_from(self.0).expect("16-bit CPU architectures are unsupported")
            }
        }

        impl From<usize> for $name {
            fn from(val: usize) -> Self {
                Self(u32::try_from(val).expect("only 32-bit GPU word sizes are supported, and given linked modules had more than 4GB of objects"))
            }
        }

        impl From<u32> for $name {
            fn from(val: u32) -> Self {
                Self(val)
            }
        }

        impl Deref for $name {
            type Target = u32;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    }
}

impl_index!(pub struct MemoryIndex);
impl_index!(pub struct TableIndex);
impl_index!(pub struct GlobalIndex);
impl_index!(pub struct ElementIndex);
impl_index!(pub struct DataIndex);
