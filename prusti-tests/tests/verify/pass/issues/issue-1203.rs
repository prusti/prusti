use prusti_contracts::*;

// Regression test for prusti/prusti#1203: reassigned `mut` arguments in postconditions.
#[ensures(result == (y, x))]
fn mut_swap(mut x: i64, mut y: i64) -> (i64, i64) {
    (x, y) = (y, x);
    (x, y)
}

#[ensures(result == x + 2)]
fn mut_add(mut x: i64) -> i64 {
    x += 2;
    x
}

#[ensures(result == x / 2)]
fn mut_div(mut x: i64) -> i64 {
    x /= 2;
    x
}

#[ensures(result == 2)]
fn mut_assign(mut x: i64) -> i64 {
    x = 2;
    x
}
