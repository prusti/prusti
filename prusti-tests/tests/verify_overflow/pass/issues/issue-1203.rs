use prusti_contracts::*;

// Regression test for prusti/prusti#1203 with overflow checks on.
#[ensures(result == (y, x))]
fn mut_swap(mut x: i64, mut y: i64) -> (i64, i64) {
    (x, y) = (y, x);
    (x, y)
}

#[requires(x < i64::MAX - 2)]
#[ensures(result == x + 2)]
fn mut_add(mut x: i64) -> i64 {
    x += 2;
    x
}
