//@ revisions: simplified unsimplified
//@[unsimplified] compile-flags: -Psimplify_encoding=false

// The quantifiers here have no triggers of their own: the verifier infers them
// from the bodies, which hold the terms the user wrote for the snapshots of a
// struct. They match the same terms whether or not the encoding is simplified.

use prusti_contracts::*;

#[derive(Clone, Copy)]
pub struct W(u32);

#[pure]
#[trusted]
fn p(x: u32) -> bool {
    true
}

#[pure]
#[trusted]
fn q(w: W) -> bool {
    true
}

// The field read `w.0` of the bound variable matches the field read of the
// constructor `W(3)`, which the simplifier keeps.
#[requires(forall(|w: W| p(w.0)))]
fn field_of_bound_variable() {
    prusti_assert!(p(W(3).0));
}

#[requires(forall(|i: u32| q(W(i))))]
fn constructor() {
    prusti_assert!(q(W(3)));
}

#[requires(forall(|i: u32| p(W(i).0)))]
fn field_of_constructor() {
    prusti_assert!(p(W(3).0));
}
