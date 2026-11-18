// Regression test for prusti/prusti#183: nested pure calls are encoded through their bodies, so
// wrong claims about them are rejected.
use prusti_contracts::*;

#[derive(Clone, Copy, PartialEq, Eq)]
struct A {
    i: i32,
}

#[pure]
fn id(x: A) -> A {
    x
}

#[pure]
fn pos(x: A) -> bool {
    x.i > 0
}

#[pure]
#[ensures(result == id(id(x)))]
fn nested_id(x: A) -> A {
    x
}

fn nested_id_is_not_arbitrary(x: A, y: A) {
    prusti_assert!(nested_id(x) == x);
    prusti_assert!(nested_id(x) == y); //~ ERROR: the asserted expression might not hold
}

fn pos_chain_is_not_always_true(x: A) {
    prusti_assert!(pos(id(x))); //~ ERROR: the asserted expression might not hold
}

#[ensures(pos(result))] //~ ERROR: postcondition might not hold
fn first_not_pos(x: A, y: A) -> A {
    id(x)
}
