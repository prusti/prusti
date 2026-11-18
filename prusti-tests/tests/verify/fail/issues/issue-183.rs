// Regression test for prusti/prusti#183: chains of pure functions are encoded with their meaning.
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

fn id_chain_is_not_changed(x: A, y: A) {
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
