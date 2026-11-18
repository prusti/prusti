//! A trait implementation that is *not* marked `#[refine_trait_spec]` carries
//! no specification of its own and inherits the trait method's specification
//! wholesale. In particular it inherits the `#[pure]` kind, so the method stays
//! usable inside pure functions and specifications without repeating `#[pure]`.

use prusti_contracts::*;

trait Trait {
    #[pure]
    fn foo(&self) -> i32;
}

struct Struct;

// No `#[refine_trait_spec]` and no `#[pure]` here: both are inherited.
impl Trait for Struct {
    fn foo(&self) -> i32 {
        5
    }
}

// Usable in a pure function only because `foo` inherited `#[pure]`.
#[pure]
fn get(s: &Struct) -> i32 {
    s.foo()
}

// Usable in a specification for the same reason.
#[requires(s.foo() == 5)]
fn requires_foo(s: &Struct) {}

// A quantifier over `Self` in the contract of a trait method, inherited by an
// impl for a type with a generic parameter of another name.
pub trait Tr {
    #[ensures(forall(|x: &Self| x === x))]
    #[ensures(result <= 10)]
    fn get(&self) -> u32;
}

pub struct W<U>(U);

impl<U> Tr for W<U> {
    fn get(&self) -> u32 {
        3
    }
}

fn call_inherited<V>(w: &W<V>) {
    let r = w.get();
    assert!(r <= 10);
}

// Inherited quantifiers whose types involve an associated type, which the impl
// defines.
pub trait TrAssoc {
    type A: Copy;
    #[ensures(forall(|x: Self::A| result <= 10))]
    fn bounded(&self) -> u32;
    #[ensures(forall(|i: u32| i == 0 ==> result === result))]
    fn value(&self) -> Self::A;
}

impl<U: Copy> TrAssoc for W<U> {
    type A = U;
    fn bounded(&self) -> u32 {
        3
    }
    fn value(&self) -> U {
        self.0
    }
}

fn call_assoc<V: Copy>(w: &W<V>) {
    let r = w.bounded();
    assert!(r <= 10);
    let _v = w.value();
}
