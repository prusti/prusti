use prusti_contracts::*;

#[ensures(old(old(*x)) == *x)] //~ERROR: `old` nested in `old`
fn nested_old(x: &mut u32) {}

#[ensures(rel0(forall(|i: u32| rel1(x) == i)))] //~ERROR: `rel` nested in `rel`
fn rel_across_closure(x: u32) {}
