// Regression test for prusti/prusti#879: chained pure functions returning tuples.
use prusti_contracts::*;

#[pure]
fn zero() -> usize {
    0
}

#[pure]
fn tpl() -> (usize, usize) {
    (zero(), zero())
}

fn wrong_component() {
    let (_, b) = tpl();
    prusti_assert!(b == 1); //~ ERROR: the asserted expression might not hold
}
