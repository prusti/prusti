// Regression test for prusti/prusti#1251: references inside old().
use prusti_contracts::*;

#[requires(a.len() > 0)]
#[ensures(&a[0] === old(&a[0]))]
fn same_element<T>(a: &[T]) {}
