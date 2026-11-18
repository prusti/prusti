// Regression test for prusti/prusti#1251: references inside old().
use prusti_contracts::*;

#[requires(a.len() > 1)]
#[ensures(&a[0] === old(&a[1]))] //~ ERROR: postcondition might not hold
fn other_element<T>(a: &[T]) {}
