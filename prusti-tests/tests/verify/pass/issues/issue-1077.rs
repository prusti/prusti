// Regression test for prusti/prusti#1077: writing through a reborrow of a field of a `&mut`
// argument.
use prusti_contracts::*;

#[ensures(x.0 == 2)]
fn write_through_reborrowed_field(x: &mut (usize, usize)) {
    let f = &mut x.0;
    *f = 1 + 1;
}
