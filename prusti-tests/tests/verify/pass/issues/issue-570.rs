//@ known-failure
// Regression test for prusti/prusti#570: `body_invariant!` after a `let`.
use prusti_contracts::*;

const N: usize = 2;
const M: usize = 4;

#[requires(forall(|i: usize| i < N ==> a[i] < M))]
fn invariant_after_let(a: &[usize; N]) {
    let mut b: [usize; M] = [0; M];
    let mut i: usize = 0;
    while i < N {
        let j = a[i];
        body_invariant!(i < N);
        body_invariant!(j < M);
        b[j] = 0;
        i = i + 1;
    }
}
