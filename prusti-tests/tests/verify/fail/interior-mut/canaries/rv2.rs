//@ compile-flags: -Passert_timeout=45000
// Review round 1: arrays of interior-mutable values are unsupported.
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

#[trusted]
#[ensures(a[0].get() == 1)]
fn set1(a: &[Cell<i32>; 1]) { //~ ERROR
    a[0].set(1)
}

#[pure]
fn first(a: &[Cell<i32>]) -> i32 { //~ ERROR
    a[0].get()
}
