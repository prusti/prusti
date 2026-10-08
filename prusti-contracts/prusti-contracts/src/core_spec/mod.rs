use crate::*;

pub mod default;
pub mod eq;
pub mod float;
pub mod panicking;
pub mod result;
pub mod slice;

pub use eq::PureEq;

pub(super) mod type_eq {
    /// A trait which can be used as a bound to say that two types are the same. For
    /// example `Self: TypeEq<Rhs>` can be used as a condition in `PartialEq`.
    #[allow(private_bounds)]
    pub trait TypeEq<T>: SealedTypeEq<T> {}
    impl<T> TypeEq<T> for T {}

    /// Makes the above trait sealed: it cannot be implemented outside this module.
    trait SealedTypeEq<T> {}
    impl<T> SealedTypeEq<T> for T {}
}
