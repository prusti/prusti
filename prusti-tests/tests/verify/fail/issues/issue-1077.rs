use prusti_contracts::*;

// Regression test for prusti/prusti#1077.
#[ensures(x.0 == 3)] //~ERROR: postcondition might not hold
fn wrong_value(x: &mut (usize, usize)) {
    let f = &mut x.0;
    *f = 1 + 1;
}

#[ensures(x.1 == old(x.1) + 1)] //~ERROR: postcondition might not hold
fn wrong_other_field(x: &mut (usize, usize)) {
    let f = &mut x.0;
    *f = 1 + 1;
}
