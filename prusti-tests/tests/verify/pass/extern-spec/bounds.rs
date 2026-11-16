//@ aux-build: compare.rs
use compare::Compare;
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
#[extern_spec(compare)]
trait Compare: Eq + PartialOrd<Self> {
    #[pure]
    #[trusted]
    fn compare(&self, other: &Self) -> std::cmp::Ordering;
}

// A by-value receiver written with its type.
#[extern_spec]
trait Not {
    #[pure]
    #[trusted]
    fn not(self: Self) -> Self::Output;
}

// `Self` returned by value, from a trait without a `Sized` supertrait.
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
}
