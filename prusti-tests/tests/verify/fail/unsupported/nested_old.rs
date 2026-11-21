use prusti_contracts::*;

#[ensures(old(old(*x)) == *x)] //~ERROR: `old` nested in `old`
fn nested_old(x: &mut u32) {}
