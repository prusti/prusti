// Regression test for prusti/prusti#796: a quantified array precondition
// is instantiated by array lookups in `assert!`.
use prusti_contracts::*;

#[requires(forall(|i: usize| (0 <= i && i < 16) ==> (array[i] > 100)))]
pub fn assert_example(array: &[usize; 16]) {
    assert!(array[0] > 100);
    assert!(array[15] > 100);
}

#[requires(forall(|i: usize| (0 <= i && i < 16) ==> (array[i] > 100)))]
pub fn assert_symbolic_index(array: &[usize; 16], k: usize) {
    if k < 16 {
        assert!(array[k] > 100);
    }
}
