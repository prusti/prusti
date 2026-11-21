use prusti_contracts::*;

#[pure]
fn fib(n: isize) -> isize {
    if n <= 1 {
        1
    } else {
        fib(n-1) + fib(n-2)
    }
}

pub fn test1() {
    // Each call unrolls the function. Hence all assertions pass.
    assert!(fib(0) == 1);
    assert!(fib(1) == 1);
    assert!(fib(2) == 2);
    assert!(fib(3) == 3);
    assert!(fib(4) == 5);
}

pub fn test2() {
    // We unroll the function at most once, hence this fails.
    assert!(fib(4) == 5);   //~ERROR: might not hold
}

#[pure]
fn dummy(_n: isize, _res: isize) -> bool { true }

#[requires(
    // This is an unsound quantifier to check whether triggerring works.
    forall(|n: isize, res: isize| fib(n) == res, triggers=[(dummy(n, res),)])
)]
pub fn test3() {
    dummy(4, 5);
    assert!(fib(4) == 5);
}

// A trigger with an indexing expression only selects instances: the bounds
// check of `a[j]` is not part of the trigger term, so it adds no facts.
#[requires(forall(|j: usize| j < 3 ==> a[j] > 0, triggers = [(a[j],)]))]
fn index_trigger_in_precondition(a: &[i32; 3]) {
    prusti_assert!(false); //~ERROR: might not hold
}

fn index_trigger_in_invariant(a: &[i32; 3]) {
    let mut i = 0;
    while i < 3 {
        body_invariant!(forall(|j: usize| j < i ==> a[j] == a[j], triggers = [(a[j],)]));
        i += 1;
    }
    prusti_assert!(false); //~ERROR: might not hold
}

#[requires(forall(|j: usize| j < a.len() ==> a[j] > 0, triggers = [(a[j],)]))]
fn slice_index_trigger(a: &[i32]) {
    prusti_assert!(false); //~ERROR: might not hold
}

// In the following functions, each quantifier's body would give the assertion,
// but no instance of any of its trigger sets is in scope, so it is not
// instantiated.

#[pure]
#[trusted]
fn f(x: i32) -> i32 { 0 }

#[pure]
#[trusted]
fn g(x: i32) -> i32 { 0 }

#[pure]
#[trusted]
fn p(x: usize) -> bool { true }

#[pure]
#[trusted]
fn q(x: usize) -> usize { 0 }

#[pure]
#[trusted]
fn r(x: usize) -> usize { 0 }

#[requires(forall(|i: i32| f(i) > 0, triggers = [(g(i),)]))]
fn restrictive() {
    prusti_assert!(f(3) > 0); //~ERROR: might not hold
}

// A trigger set only matches if all of its terms do.
#[requires(forall(|i: usize| p(i), triggers = [(q(i), r(i))]))]
#[requires(q(0) == 1)]
fn one_term_of_set() {
    prusti_assert!(p(0)); //~ERROR: might not hold
}

// No `q` term is in scope to prove the `exists` through its trigger.
#[requires(p(0))]
fn existential() {
    prusti_assert!(exists(|i: usize| p(i), triggers = [(q(i),)])); //~ERROR: might not hold
}

#[pure]
#[trusted]
fn at(n: usize, i: usize) -> usize { 0 }

#[trusted]
#[ensures(forall(|i: usize| p(i), triggers = [(at(old(n), i),)]))]
fn establish(n: usize) {}

// The trigger refers to the argument of the call, 3, not to 4.
#[requires(at(4, 5) == 0)]
fn old_in_trigger() {
    establish(3);
    prusti_assert!(p(5)); //~ERROR: might not hold
}

// Neither of the alternative trigger sets matches.
#[requires(forall(|i: usize| p(i), triggers = [(q(i),), (r(i),)]))]
fn no_set_matches() {
    prusti_assert!(p(0)); //~ERROR: might not hold
}

// Triggers with a runtime check (indexing, division) are honoured too, rather
// than replaced by a trigger Viper chooses.
#[requires(1 < a.len())]
#[requires(forall(|i: usize| i < a.len() ==> p(i), triggers = [(a[i],)]))]
fn slice_index(a: &[i32]) {
    prusti_assert!(p(0)); //~ERROR: might not hold
}

// The divisor is known to be nonzero: Silicon checks the precondition of the
// division when it evaluates the trigger, and would otherwise discard the
// trigger and choose one itself, under which the assertion holds.
#[requires(d > 0)]
#[requires(forall(|i: usize| i < 3 ==> p(i), triggers = [(q(i / d),)]))]
fn division(d: usize) {
    prusti_assert!(p(0)); //~ERROR: might not hold
}
