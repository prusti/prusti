// Regression test for prusti/prusti#1203: reassigned `mut` arguments in postconditions, with
// overflow checks on.
use prusti_contracts::*;

#[requires(x < i64::MAX - 2)]
#[ensures(result == x + 2)]
fn mut_add(mut x: i64) -> i64 {
    x += 2;
    x
}
