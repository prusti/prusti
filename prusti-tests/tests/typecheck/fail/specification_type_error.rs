// This test checks that specifications are type-checked.

use prusti_contracts::*;

#[ensures(x != true)]  //~ERROR: mismatched types
pub fn first_1(x: i32, y: i32) -> i32 {
    if x != true {     //~ERROR: mismatched types
    }
    x
}

pub fn test_error_reporting(x: i32) -> bool {
    x     //~ERROR: mismatched types
}

pub struct S;

#[pure]
#[trusted]
fn by_value(s: S, i: usize) -> bool {
    true
}

#[pure]
#[trusted]
fn p(i: usize) -> bool {
    true
}

// A trigger, like the body of a quantifier, can only read the variables it
// captures; this one moves `s` into the call.
#[requires(forall(|i: usize| p(i), triggers = [(by_value(s, i),)]))] //~ERROR: expected a closure that implements the `Fn` trait
pub fn trigger_moves_capture(s: S) {}

#[requires(forall(|i: usize| p(i), triggers = [(p(i), p(i), p(i), p(i), p(i), p(i), p(i), p(i), p(i), p(i), p(i), p(i), p(i))]))] //~ERROR: a trigger set can have at most 12 expressions
pub fn too_many_terms() {}

// The return type becomes the type of the postcondition's `result` parameter,
// where precise capturing is not allowed.
#[ensures(true)]
pub fn precise_capture() -> impl Sized + use<> { //~ERROR: `use<...>` precise capturing syntax not allowed in argument-position
    0
}

// TODO: Test parser errors.
