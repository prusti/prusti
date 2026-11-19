use prusti_contracts::*;

#[pure]
#[trusted]
fn len(a: u32) -> usize { 0 }

#[pure]
#[trusted]
#[requires(i < len(a))]
fn lookup(a: u32, i: usize) -> i32 { 0 }

// A trigger mentioning a captured variable and the bound variable matches the
// term in the assertion, so the quantifier is instantiated.
#[requires(forall(|i: usize| i < len(a) ==> lookup(a, i) > 0, triggers = [(lookup(a, i),)]))]
#[requires(len(a) > 3)]
fn instantiated(a: u32) {
    prusti_assert!(lookup(a, 2) > 0);
}

// A trigger set with two terms over two bound variables, chained through a
// quantifier relating `b` to `a`: proving `b` sorted needs `lookup(b, _)` to
// bring `lookup(a, _)` into scope, which in turn fires the sortedness of `a`.
#[requires(len(a) == len(b))]
#[requires(forall(|i: usize, j: usize| (i < j && j < len(a)) ==> lookup(a, i) <= lookup(a, j),
                  triggers = [(lookup(a, i), lookup(a, j))]))]
#[requires(forall(|i: usize| i < len(a) ==> lookup(b, i) == lookup(a, i),
                  triggers = [(lookup(b, i),)]))]
fn framed(a: u32, b: u32) {
    prusti_assert!(forall(|i: usize, j: usize| (i < j && j < len(b)) ==> lookup(b, i) <= lookup(b, j),
                          triggers = [(lookup(b, i), lookup(b, j))]));
}

#[pure]
#[trusted]
fn p(x: usize) -> bool { true }

#[pure]
#[trusted]
fn q(x: usize) -> usize { 0 }

#[pure]
#[trusted]
fn r(x: usize) -> usize { 0 }

// Of several trigger sets, any one that matches instantiates the quantifier:
// here only `r(0)` is in scope.
#[requires(forall(|i: usize| p(i), triggers = [(q(i),), (r(i),)]))]
#[requires(r(0) == 1)]
fn second_set_matches() {
    prusti_assert!(p(0));
}

// A trigger set with several terms matches once all of them are in scope.
#[requires(forall(|i: usize| p(i), triggers = [(q(i), r(i))]))]
#[requires(q(0) == 1 && r(0) == 1)]
fn all_terms_of_set() {
    prusti_assert!(p(0));
}

// An `exists` is proved through its trigger, here by the term `q(0)`.
#[requires(p(0) && q(0) == 0)]
fn existential() {
    prusti_assert!(exists(|i: usize| p(i), triggers = [(q(i),)]));
}

#[pure]
#[trusted]
fn s(x: i32, y: i32) -> bool { true }

#[pure]
#[trusted]
fn at(n: usize, i: usize) -> usize { 0 }

#[trusted]
#[ensures(forall(|i: usize| p(i), triggers = [(at(old(n), i),)]))]
fn establish(n: usize) {}

// A trigger may refer to the state before a call with `old`.
#[requires(at(3, 5) == 0)]
fn old_in_trigger() {
    establish(3);
    prusti_assert!(p(5));
}

// A trigger may contain a negative literal.
#[requires(forall(|i: i32| s(i, -1), triggers = [(s(i, -1),)]))]
fn negative_literal() {
    prusti_assert!(s(3, -1));
}
