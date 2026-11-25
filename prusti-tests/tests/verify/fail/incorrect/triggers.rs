use prusti_contracts::*;

#[pure]
#[trusted]
fn baz(slice: &[i32]) -> bool {
    true
}

// Every trigger of a trigger set must mention a bound variable, not just the set
// as a whole: Viper's rules reject `slice.len()` and `baz(slice)` here.
#[requires(forall(|i: usize| i < slice.len() ==> slice[i] > 10, triggers = [(slice[i], slice.len(), baz(slice))]))] //~ERROR: every expression of a trigger set must mention a bound variable of the quantifier
fn term_without_bound_variable(slice: &[i32]) {}

#[pure]
#[trusted]
fn p(i: usize) -> bool {
    true
}

#[requires(forall(|i: usize| p(i), triggers = [(i,)]))] //~ERROR: a trigger must be a call, an operator, a constructor, a field access, an indexing or a cast, not a bare variable or constant
fn bare_variable() {}

// A value from outside the quantifier, also behind a reference or as the
// field of a variable (a closure captures such a field by itself, a disjoint
// field capture).
#[requires(forall(|i: usize| p(i), triggers = [(n,)]))] //~ERROR: not a value from outside the quantifier
fn captured_variable(n: usize) {}

#[requires(forall(|i: usize| p(i), triggers = [(*r,)]))] //~ERROR: not a value from outside the quantifier
fn dereferenced_capture(r: &usize) {}

pub struct S {
    pub len: usize,
}

#[requires(forall(|i: usize| p(i), triggers = [(s.len,)]))] //~ERROR: not a value from outside the quantifier
fn captured_field(s: S) {}

// The simplifier folds away borrows, so a trigger cannot be one.
#[requires(forall(|i: usize| i < s.len() ==> p(i), triggers = [(&s[i],)]))] //~ERROR: a trigger cannot be a borrow
fn borrow(s: &[usize]) {}

#[requires(forall(|i: usize, j: usize| q2(i, j), triggers = [((i, j),)]))] //~ERROR: a trigger cannot be a tuple or a closure
fn tuple() {}

#[requires(forall(|i: usize| p(i), triggers = [((p(i), 0usize).0,)]))] //~ERROR: a trigger cannot be a field of a tuple or a closure it builds
fn field_of_tuple() {}

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

// An operation at the top of a trigger gets the same error, not the error
// about the outermost operation.
#[requires(forall(|i: Int| true, triggers = [(i + Int::from(1),)]))] //~ERROR: a trigger cannot contain the snapshot equality `===` or an operation on Prusti's mathematical integers and reals (`Int`, `Real`)
fn int_operation_at_top() {}

#[requires(forall(|i: usize| p(i), triggers = [(of_bool(i > 3 && i < 5),)]))] //~ERROR: a trigger cannot contain a conditional
fn conditional() {}

#[requires(forall(|i: usize| p(i), triggers = [(of_bool(forall(|j: usize| p(j + i))),)]))] //~ERROR: a trigger cannot contain a quantifier
fn quantifier() {}

// Of the states other than the current one, only `old` is supported in a
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
