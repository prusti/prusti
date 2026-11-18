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

fn destructure_tuple() {
    let (a, b) = tpl();
    assert!(a == 0 && b == 0);
}
