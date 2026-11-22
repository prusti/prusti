//@ aux-build: foreign_traits.rs
use foreign_traits::{Compare, Convert, Other, Pair, Plain, Projected, Wrapped};
use prusti_contracts::*;
use std::{cell::Cell, ops::Not};

// Extern specs whose declared bounds follow from the target's.

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

// Methods that take or return `Self` by value. Their spec items take it by value
// too, so they need `Self: Sized`, as the spec of a method of a local trait
// does. The target has it at every use: `Self` as such, as the last element of
// a tuple, or as the last field of a struct.
#[extern_spec(foreign_traits)]
trait Pair {
    #[trusted]
    fn pair(&self) -> (u8, Self)
    where
        Self: Sized;
}

#[extern_spec(foreign_traits)]
trait Wrapped {
    #[trusted]
    fn wrapped(x: Cell<Self>) -> bool
    where
        Self: Sized;
}

// `T` taken by value through a projection that normalizes to it.
#[extern_spec(foreign_traits)]
trait Projected<T: ?Sized> {
    #[trusted]
    fn take(&self, x: <Self as Other>::A)
    where
        T: Sized;
}

// A by-value receiver written with its type.
#[extern_spec]
trait Not {
    #[pure]
    #[trusted]
    fn not(self: Self) -> Self::Output
    where
        Self: Sized;
}

// From a trait without a `Sized` supertrait.
#[cfg(unix)]
#[extern_spec(std::os::fd)]
trait FromRawFd {
    #[trusted]
    unsafe fn from_raw_fd(fd: std::os::fd::RawFd) -> Self
    where
        Self: Sized;
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
