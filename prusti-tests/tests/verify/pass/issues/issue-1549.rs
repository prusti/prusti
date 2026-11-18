// Regression test for prusti/prusti#1549: references to constants as arguments.
use prusti_contracts::*;

#[requires(*y == 5)]
#[ensures(result == x / 5)]
fn div(x: u64, y: &u64) -> u64 {
    let y = *y;
    x / y
}

#[ensures(result == 2)]
fn call_const_ref() -> u64 {
    div(10, &5)
}
