use prusti_contracts::*;

#[pure]
#[trusted]
fn baz(slice: &[i32]) -> bool {
    true
}

// Every term of a trigger set must mention a bound variable, not just the set
// as a whole: Viper rejects `slice.len()` and `baz(slice)` here, and Z3 never
// instantiates a quantifier through a multi-pattern with a term without one.
#[requires(forall(|i: usize| i < slice.len() ==> slice[i] > 10, triggers = [(slice[i], slice.len(), baz(slice))]))] //~ERROR: every expression of a trigger set must mention a bound variable of the quantifier
fn term_without_bound_variable(slice: &[i32]) {}

#[pure]
#[trusted]
fn p(i: usize) -> bool {
    true
}

#[requires(forall(|i: usize| p(i), triggers = [(i,)]))] //~ERROR: a trigger must be a function or method call, an operator, a field access or an indexing, not a bare variable or constant
fn bare_variable() {}

#[requires(forall(|i: usize| p(i), triggers = [()]))] //~ERROR: a trigger set must contain at least one expression
fn empty_set() {}

#[pure]
#[trusted]
fn q2(i: usize, j: usize) -> bool {
    true
}

#[requires(forall(|i: usize, j: usize| q2(i, j), triggers = [(p(i), p(i))]))] //~ERROR: this trigger set does not mention the bound variable `j`
fn set_without_variable() {}

#[pure]
#[trusted]
fn of_bool(b: bool) -> bool {
    true
}

// Snapshot equality is one of Viper's own operators, which a trigger cannot
// contain.
#[requires(forall(|i: usize| p(i), triggers = [(of_bool(i === 1),)]))] //~ERROR: a trigger cannot contain the snapshot equality `===` or an operation on Prusti's mathematical integers and reals (`Int`, `Real`)
fn snapshot_equality() {}

#[pure]
#[trusted]
fn of_real(r: Real) -> bool {
    true
}

#[requires(forall(|r: Real| of_real(r), triggers = [(of_real(r + Real::from(1.0f64)),)]))] //~ERROR: a trigger cannot contain the snapshot equality `===` or an operation on Prusti's mathematical integers and reals (`Int`, `Real`)
fn real_operation() {}

// An operation at the top of a trigger gets the same error, not the one for a
// trigger that is no call.
#[requires(forall(|i: Int| true, triggers = [(i + Int::from(1),)]))] //~ERROR: a trigger cannot contain the snapshot equality `===` or an operation on Prusti's mathematical integers and reals (`Int`, `Real`)
fn int_operation_at_top() {}

#[requires(forall(|i: usize| p(i), triggers = [(of_bool(i > 3 && i < 5),)]))] //~ERROR: a trigger cannot contain a conditional
fn conditional() {}

#[requires(forall(|i: usize| p(i), triggers = [(of_bool(forall(|j: usize| p(j + i))),)]))] //~ERROR: a trigger cannot contain a quantifier
fn quantifier() {}

// Of the states other than the current one, only `old` can be read in a
// trigger.
#[requires(forall(|i: usize| p(i), triggers = [(p(before_expiry(i)),)]))] //~ERROR: `before_expiry` in a trigger
fn before_expiry_in_trigger() {}

#[pure]
#[trusted]
fn h(i: usize, j: usize) -> bool {
    true
}

#[requires(forall(|i: usize| p(i), triggers = [(h(i, panic!()),)]))] //~ERROR: a trigger cannot contain a call that never returns, such as `panic!()`
fn diverging() {}
