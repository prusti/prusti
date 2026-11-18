// Regression test for prusti/prusti#1203: reassigned `mut` arguments in postconditions, with
// overflow checks on.
use prusti_contracts::*;

#[ensures(result == x + 2)]
fn mut_add_unbounded(mut x: i64) -> i64 {
    x += 2; //~ ERROR: attempt to add with overflow
    x
}
