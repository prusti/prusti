// Regression test for prusti/prusti#1325: nested quantifiers.
use prusti_contracts::*;

#[requires(forall(|a: i32| a > 0 ==> exists(|b: i32| b > a && b < a + 2)))]
fn nested_quantifier_wrong_claim(x: i32) {
    prusti_assert!(x > 0 ==> exists(|b: i32| b > x + 5 && b < x + 3)); //~ ERROR: the asserted expression might not hold
}
