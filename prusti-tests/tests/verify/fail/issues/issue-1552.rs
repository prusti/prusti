use prusti_contracts::*;

// Regression test for prusti/prusti#1552.
#[pure]
fn decrease(value: usize) -> usize {
    value - 1
}

#[requires(value > 0)]
#[ensures(decrease(value) == value)] //~ ERROR: postcondition might not hold
fn use_decrease_wrong(value: usize) {}
