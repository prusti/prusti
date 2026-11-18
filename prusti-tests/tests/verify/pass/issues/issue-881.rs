use prusti_contracts::*;

// Regression test for prusti/prusti#881: negation is not checked when overflow checks are off.
#[ensures(result == -x)]
fn negate(x: i32) -> i32 {
    -x
}
