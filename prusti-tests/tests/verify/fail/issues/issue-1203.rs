use prusti_contracts::*;

// Regression test for prusti/prusti#1203.
#[ensures(result == (x, y))] //~ ERROR: postcondition might not hold
fn mut_swap_wrong(mut x: i64, mut y: i64) -> (i64, i64) {
    (x, y) = (y, x);
    (x, y)
}

#[ensures(result == x + 3)] //~ ERROR: postcondition might not hold
fn mut_add_wrong(mut x: i64) -> i64 {
    x += 2;
    x
}
