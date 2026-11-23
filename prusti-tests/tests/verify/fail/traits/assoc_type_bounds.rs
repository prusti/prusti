// The bounds declared on an associated type hold of it, and nothing more.
// With specialization, `f` can be called on any `Wrapper<T>`, and only
// resolves to the impl ensuring `result == 5` where `T: Copy` holds.

#![feature(specialization)]
#![allow(incomplete_features)]

use prusti_contracts::*;

pub struct Wrapper<T>(T);

pub trait Tr {
    #[pure]
    fn f(&self) -> u32;
}

#[refine_trait_spec]
impl<T> Tr for T {
    #[pure]
    default fn f(&self) -> u32 {
        0
    }
}

#[refine_trait_spec]
impl<T: Copy> Tr for Wrapper<T> {
    #[pure]
    #[ensures(result == 5)]
    fn f(&self) -> u32 {
        5
    }
}

pub trait Model {
    type State: Copy;
}

pub fn bounded<M: Model>(w: &Wrapper<M::State>) {
    assert!(w.f() == 5);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

pub trait Unbounded {
    type State;
}

pub fn unbounded<M: Unbounded>(w: &Wrapper<M::State>) {
    assert!(w.f() == 5); //~ ERROR: precondition of `core::panicking::panic`
}

pub trait Family {
    type Member<X>: Copy
    where
        X: Copy;
}

pub fn gat<F: Family>(w: &Wrapper<F::Member<u8>>) {
    assert!(w.f() == 5);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

fn main() {}
