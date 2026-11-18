// Regression test for prusti/prusti#796: a quantified array precondition
// is instantiated by array lookups in `assert!`, but only within its range.
use prusti_contracts::*;

#[requires(forall(|i: usize| (0 <= i && i < 16) ==> (array[i] > 100)))]
pub fn stronger_bound(array: &[usize; 16]) {
    assert!(array[0] > 101); //~ ERROR: precondition of `core::panicking::panic`
}

#[requires(forall(|i: usize| (0 <= i && i < 8) ==> (array[i] > 100)))]
pub fn index_outside_range(array: &[usize; 16]) {
    assert!(array[10] > 100); //~ ERROR: precondition of `core::panicking::panic`
}
