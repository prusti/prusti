//@ revisions: simplified unsimplified
//@[unsimplified] compile-flags: -Psimplify_encoding=false

// The quantifiers here have no triggers of their own: the verifier infers them
// from the bodies, which hold the terms the user wrote for the snapshots of a
// struct and for the variables the quantifier captures. They match the same
// terms whether or not the encoding is simplified.

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

#[pure]
#[trusted]
fn at(n: u32, i: u32) -> u32 {
    0
}

#[pure]
#[trusted]
fn at_ref(n: &u32, i: u32) -> u32 {
    0
}

// A quantifier reads a variable it captures as the variable itself, so the
// trigger `at(n, i)` matches `at(n, 5)`.
#[requires(forall(|i: u32| at(n, i) == 0 ==> p(i)))]
#[requires(at(n, 5) == 0)]
fn capture(n: u32) {
    prusti_assert!(p(5));
}

// The quantifier borrows the capture.
#[requires(forall(|i: u32| at_ref(&n, i) == 0 ==> p(i)))]
#[requires(at_ref(&n, 5) == 0)]
fn capture_borrowed(n: u32) {
    prusti_assert!(p(5));
}

// The quantifier captures the slice `*s`, whose length is in the metadata of
// `s`.
#[requires(forall(|i: usize| i < s.len() ==> s[i] == 0))]
#[requires(s.len() == 3)]
fn capture_unsized(s: &[u32]) {
    prusti_assert!(s[1] == 0);
}

// A quantifier inside a `prusti_assert!` captures what that closure captured.
#[requires(forall(|i: u32| at(n, i) == 0))]
fn capture_nested(n: u32) {
    prusti_assert!(forall(|i: u32| at(n, i) == 0));
}

// The quantifier reads a capture in `old`.
#[ensures(forall(|i: u32| at(*x, i) == old(at(*x, i))))]
fn frame(x: &mut u32) {}

#[requires(at(*x, 4) == 7)]
fn capture_old(x: &mut u32) {
    frame(x);
    prusti_assert!(at(*x, 4) == 7);
}
