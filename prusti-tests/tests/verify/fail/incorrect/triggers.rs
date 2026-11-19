use prusti_contracts::*;

#[pure]
#[trusted]
fn baz(slice: &[i32]) -> bool {
    true
}

// Every term of a trigger set must mention a bound variable, not just the set
// as a whole: Viper rejects `slice.len()` and `baz(slice)` here, and Z3 never
// instantiates a quantifier through a multi-pattern with a term without one.
#[requires(forall(|i: usize| i < slice.len() ==> slice[i] > 10, triggers = [(slice[i], slice.len(), baz(slice))]))] //~ERROR: must mention a variable bound by its quantifier
fn term_without_bound_variable(slice: &[i32]) {}

#[pure]
#[trusted]
fn p(i: usize) -> bool {
    true
}

#[requires(forall(|i: usize| p(i), triggers = [(i,)]))] //~ERROR: a trigger must be a function or method call
fn bare_variable() {}

#[requires(forall(|i: usize| p(i), triggers = [()]))] //~ERROR: a trigger set must contain at least one term
fn empty_set() {}

#[pure]
#[trusted]
fn q2(i: usize, j: usize) -> bool {
    true
}

#[requires(forall(|i: usize, j: usize| q2(i, j), triggers = [(p(i), p(i))]))] //~ERROR: this trigger set does not mention the quantified variable `j`
fn set_without_variable() {}

#[pure]
#[trusted]
fn of_bool(b: bool) -> bool {
    true
}

// Snapshot equality is one of Viper's own operators, which a trigger cannot
// contain.
#[requires(forall(|i: usize| p(i), triggers = [(of_bool(i === 1),)]))] //~ERROR: a trigger cannot contain `===`
fn snapshot_equality() {}

#[requires(forall(|i: usize| p(i), triggers = [(of_bool(i > 3 && i < 5),)]))] //~ERROR: a trigger cannot contain a conditional
fn conditional() {}

#[requires(forall(|i: usize| p(i), triggers = [(of_bool(forall(|j: usize| p(j + i))),)]))] //~ERROR: a trigger cannot contain a quantifier
fn quantifier() {}

// Of the states other than the current one, only `old` can be read in a
// trigger.
#[requires(forall(|i: usize| p(i), triggers = [(p(before_expiry(i)),)]))] //~ERROR: `rel` and `before_expiry` in a trigger
fn before_expiry_in_trigger() {}
