use prusti_contracts::*;

// Regression test for prusti/prusti#1077.
#[ensures(x.0 == 2)]
fn write_through_reborrowed_field(x: &mut (usize, usize)) {
    let f = &mut x.0;
    *f = 1 + 1;
}
