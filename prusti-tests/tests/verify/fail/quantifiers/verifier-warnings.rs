//@ compile-flags: -Wwarnings
// The verifier's warnings are reported at their position. The harness
// silences warnings (`-Awarnings`), which `-Wwarnings` undoes.
use prusti_contracts::*;

// The verifier finds no term in the quantifier's body to use as a trigger.
// The fail group needs an error besides the warning.
#[requires(forall(|a: i32| exists(|b: i32| b == 5 ==> b < a)))] //~WARN: the verifier found no trigger for this quantifier, so it might not be able to use it
pub fn no_trigger() {
    prusti_assert!(false); //~ERROR: might not hold
}

#[pure]
#[trusted]
pub fn p(_x: usize) -> bool {
    true
}

#[pure]
#[trusted]
pub fn q(_x: usize) -> usize {
    0
}

// The trigger does not occur in the body, so it is evaluated on its own,
// where the divisor `i` is not known to be nonzero.
#[requires(forall(|i: usize| p(i), triggers = [(q(10 / i),)]))] //~WARN: this expression is left out of its trigger set
pub fn division() {}

// The trigger reads the captured `n`, but it is the same term as the body
// contains, so it is evaluated with the body's guard, without a warning.
#[requires(forall(|i: usize| i > 0 ==> p(i) && q(n / i) == q(n / i), triggers = [(q(n / i),)]))]
pub fn guarded_division(n: usize) -> usize {
    n
}

// A trigger without an operation that has a precondition gets no warning and
// is used: no `q(n + 2)` term is in scope, so the quantifier is not
// instantiated.
#[requires(forall(|i: usize| p(i) && q(n + i) == q(n + i), triggers = [(q(n + i),)]))]
pub fn addition(n: usize) -> usize {
    prusti_assert!(p(2)); //~ERROR: might not hold
    n
}
