use prusti_contracts::*;

// Regression test for prusti/prusti#1203 with overflow checks on.
#[ensures(result == x + 2)]
fn mut_add_unbounded(mut x: i64) -> i64 {
    x += 2; //~ ERROR: attempt to add with overflow
    x
}
