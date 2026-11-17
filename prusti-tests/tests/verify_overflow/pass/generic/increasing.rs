//@ known-failure
use prusti_contracts::*;

predicate! {
    fn increasing<T: PartialOrd>(s: &[T]) -> bool {
        forall(|i: usize, j: usize|
            (i < j && j < s.len() ==> s[i] < s[j]))
    }
}

fn test() {
    let x = [1, 2, 3];
    prusti_assert!(increasing(&x));
}
