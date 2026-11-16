//@ aux-build: foreign_traits.rs
use foreign_traits::{Compare, Convert, Pair, Plain};
use prusti_contracts::*;
use std::{fmt::Debug, ops::Not};

// Extern specs whose declared bounds follow from the target's.

// The impl's parameters in a different order than the target's.
#[extern_spec]
impl<E, T> Result<T, E> {
    #[trusted]
    fn unwrap(self) -> T
    where
        E: Debug;
}

// `Clone` follows from the target's `T: Copy`.
#[extern_spec]
impl<T: Copy + Clone> Option<&T> {
    #[pure]
    #[trusted]
    fn copied(self) -> Option<T>;
}

// The supertraits of the target, including one that mentions `Self`.
#[extern_spec(foreign_traits)]
trait Compare: Eq + PartialOrd<Self> {
    #[pure]
    #[trusted]
    fn compare(&self, other: &Self) -> std::cmp::Ordering;
}

// The impl method only requires `U: Sized`, the trait method `U: Clone`.
#[extern_spec]
impl Convert for Plain {
    #[trusted]
    #[ensures(result)]
    fn convert<U: Clone>(&self, u: U) -> bool;
}

// `Self` as the last element of a returned tuple.
#[extern_spec(foreign_traits)]
trait Pair {
    #[trusted]
    fn pair(&self) -> (u8, Self);
}

// A by-value receiver written with its type.
#[extern_spec]
trait Not {
    #[pure]
    #[trusted]
    fn not(self: Self) -> Self::Output;
}

// `Self` returned by value, from a trait without a `Sized` supertrait.
#[cfg(unix)]
#[extern_spec(std::os::fd)]
trait FromRawFd {
    #[trusted]
    unsafe fn from_raw_fd(fd: std::os::fd::RawFd) -> Self;
}

// A bound on a projection that the target states on an equal projection.
#[extern_spec]
trait Iterator {
    #[trusted]
    fn cmp<I>(self, other: I) -> std::cmp::Ordering
    where
        I: IntoIterator<Item = Self::Item>,
        I::Item: Ord,
        Self: Sized;

    // A higher-ranked bound, as in the target.
    #[trusted]
    fn filter<P>(self, predicate: P) -> std::iter::Filter<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool;
}
