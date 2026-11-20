//@ compile-flags: -Wwarnings
// The verifier's warnings about triggers are reported.
use prusti_contracts::*;

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
