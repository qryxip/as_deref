#![no_std]

pub use as_deref_derive::AsDeref;

pub trait AsDeref {
    type Target;
    fn as_deref(self) -> Self::Target;
}

pub trait AsDerefMut {
    type Target;
    fn as_deref_mut(self) -> Self::Target;
}
