// Regression test for prusti/prusti#1325: nested quantifiers.
use prusti_contracts::*;

#[requires(forall(|a: i32| exists(|b: i32| b == 5 ==> b < a)))]
fn nested_quantifier_precondition(x: i32) -> i32 {
    0
}

#[requires(forall(|a: i32| a > 0 ==> exists(|b: i32| b > a && b < a + 2)))]
fn nested_quantifier_instantiated(x: i32) {
    prusti_assert!(x > 0 ==> exists(|b: i32| b > x && b < x + 3));
}
