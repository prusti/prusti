use prusti_contracts::*;
// The builtin under another name, which the specification parser leaves alone,
// to build a quantifier's closure apart from the quantifier.
use prusti_contracts::forall as quantify;

// The specification macros build a quantifier's closure right before the
// quantifier, in the same block and with no variable assigned in between. A
// closure built elsewhere is not supported.

#[requires({
    let c = if n > 0 { Some(move |i: u32| (i < n) | (i >= n)) } else { None };
    match c {
        Some(c) => quantify((), &c), //~ ERROR: a `forall` whose closure is not built right before it
        None => true,
    }
})]
fn built_in_branch(n: u32) {}

#[requires({
    let mut r = a;
    let c = |i: u32| (i >= *r) | (i < 10);
    r = b;
    quantify((), &c) //~ ERROR: a `forall` whose closure is not built right before it
})]
fn variable_assigned_in_between(a: &u32, b: &u32) {}

// A `move` closure captures the reference `r` by value, which the
// specification syntax never does.
#[requires(quantify((), &(move |i: u32| i != *r)))] //~ ERROR: a reference captured by value in a spec closure
fn reference_captured_by_value(r: &u32) {}
