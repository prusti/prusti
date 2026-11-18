use prusti_contracts::*;

// Regression test for prusti/prusti#1190: every stacked pledge is checked.
#[after_expiry(*x == before_expiry(*result))]
#[after_expiry(*x == 7)] //~ERROR: pledge in the postcondition might not hold
fn second_pledge_false(x: &mut i32) -> &mut i32 {
    x
}
