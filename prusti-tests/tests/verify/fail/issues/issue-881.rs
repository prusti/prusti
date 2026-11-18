use prusti_contracts::*;

// Regression test for prusti/prusti#881.
#[ensures(result == x)] //~ ERROR: postcondition might not hold
fn negate_wrong(x: i32) -> i32 {
    -x
}
