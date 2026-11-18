// Regression test for prusti/prusti#1251: references inside old().
use prusti_contracts::*;

#[requires(a.len() > 1)]
#[ensures(&a[0] === old(&a[0]))]
fn same_element<T>(a: &[T]) {}

#[requires(a.len() > 1)]
fn use_postcondition<T>(a: &[T]) {
    same_element(a);
}
