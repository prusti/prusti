// Regression test for prusti/prusti#569: non-consecutive `body_invariant!`s.
use prusti_contracts::*;

const N: usize = 8;
const M: usize = 4;

#[requires(forall(|i: usize| i < N ==> a[i] < M))]
fn separated_invariants(a: &[usize; N]) {
    let mut i: usize = 0;
    while i < N {
        body_invariant!(i < N);
        let j = a[i];
        body_invariant!(j < M); //~ ERROR: `body_invariant!` annotation may not be reached in every loop iteration
        i = i + 1;
    }
}
