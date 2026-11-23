use prusti_contracts::*;
// The builtin under another name, which the specification parser leaves alone,
// to build a quantifier's closure apart from the quantifier.
use prusti_contracts::forall as quantify;

// A quantifier's captures are encoded where its closure is built, which must be
// on the way to the quantifier: a closure built in a conditional that ends
// before the quantifier is not supported.

#[requires({
    let c = if n > 0 { Some(move |i: u32| (i < n) | (i >= n)) } else { None };
    match c {
        Some(c) => quantify((), &c), //~ ERROR: a `forall` whose closure is built in a conditional that ends before it, or not in this body
        None => true,
    }
})]
fn built_in_branch(n: u32) {}

// A `move` closure captures the reference `r` by value, which the
// specification syntax never does.
#[requires(quantify((), &(move |i: u32| i != *r)))] //~ ERROR: a reference captured by value in a spec closure
fn reference_captured_by_value(r: &u32) {}
