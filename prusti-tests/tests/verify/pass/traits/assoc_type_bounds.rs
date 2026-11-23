// The bounds declared on an associated type hold of it wherever its trait
// is implemented.

use prusti_contracts::*;

pub struct Wrapper<T>(T);

pub trait Tr {
    #[pure]
    fn f(&self) -> u32;
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

pub fn via_param<T: Copy>(w: &Wrapper<T>) {
    assert!(w.f() == 5);
}

pub fn via_assoc<M: Model>(w: &Wrapper<M::State>) {
    assert!(w.f() == 5);
}

pub fn via_where<M: Model>(w: &Wrapper<M::State>)
where
    M::State: Copy,
{
    assert!(w.f() == 5);
}

// A bound on a nested associated type, given through an associated-type
// binding.
pub trait Outer {
    type Inner: Model<State: Copy>;
}

pub fn via_nested<O: Outer>(w: &Wrapper<<O::Inner as Model>::State>) {
    assert!(w.f() == 5);
}

// A projection bound.
pub trait Keyed {
    type Key;
}

pub struct KeyWrapper<T>(T);

#[refine_trait_spec]
impl<T: Keyed<Key = u32>> Tr for KeyWrapper<T> {
    #[pure]
    #[ensures(result == 7)]
    fn f(&self) -> u32 {
        7
    }
}

pub trait Store {
    type Entry: Keyed<Key = u32>;
}

pub fn via_projection<S: Store>(w: &KeyWrapper<S::Entry>) {
    assert!(w.f() == 7);
}

// A bound mentioning the trait itself.
pub trait Chain {
    type Next: Chain + Copy;
}

pub fn via_recursive<C: Chain>(w: &Wrapper<<C::Next as Chain>::Next>) {
    assert!(w.f() == 5);
}

// A bound on a generic associated type, under its where-clause.
pub trait Family {
    type Member<X>: Copy
    where
        X: Copy;
}

pub fn via_gat<F: Family>(w: &Wrapper<F::Member<u8>>) {
    assert!(w.f() == 5);
}

// The bound holds of an impl's associated type.
#[derive(Clone, Copy)]
pub struct S;

pub struct M1;

impl Model for M1 {
    type State = S;
}

pub fn concrete(w: &Wrapper<<M1 as Model>::State>) {
    via_assoc::<M1>(w);
}

fn main() {}
