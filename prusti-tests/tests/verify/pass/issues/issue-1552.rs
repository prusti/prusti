use prusti_contracts::*;

// Regression test for prusti/prusti#1552: no internal error for a pure unsigned subtraction
// when overflow checks are off.
#[pure]
fn decrease(value: usize) -> usize {
    value - 1
}

#[requires(value > 0)]
#[ensures(decrease(value) + 1 == value)]
fn use_decrease(value: usize) {}

fn decrease_zero() {
    decrease(0);
}
