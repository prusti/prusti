// Array and slice indexing in quantifier triggers. The functions with an
// assertion prove it by instantiating the quantifier through the trigger;
// `caller` proves the quantifier in the precondition of `callee`.
use prusti_contracts::*;

#[requires(2 < slice.len())]
#[requires(forall(|i: usize| i < slice.len() ==> slice[i] > 10, triggers = [(slice[i],)]))]
fn slice_index(slice: &[i32]) {
    prusti_assert!(slice[2] > 10);
}

#[requires(forall(|i: usize| i < array.len() ==> array[i] > 10, triggers = [(array[i],)]))]
fn array_index(array: &[i32; 10]) {
    prusti_assert!(array[2] > 10);
}

#[pure]
#[trusted]
fn bar(a: i32) -> bool { true }

// An indexing expression as the argument of a call.
#[requires(1 < slice.len())]
#[requires(forall(|i: usize| i < slice.len() ==> bar(slice[i]), triggers = [(bar(slice[i]),)]))]
fn index_in_call(slice: &[i32]) {
    prusti_assert!(bar(slice[1]));
}

#[requires(forall(|i: usize| i < slice.len() ==> bar(slice[i]), triggers = [(bar(slice[i]),)]))]
fn callee(slice: &[i32]) {}

// A caller establishing a precondition with an indexing trigger.
#[requires(slice.len() == 1)]
fn caller(slice: &[i32]) {
    if bar(slice[0]) {
        callee(slice);
    }
}

// A trigger set with several terms, each mentioning the bound variable.
#[requires(1 < a.len() && a.len() == b.len())]
#[requires(forall(|i: usize| i < a.len() ==> a[i] == b[i], triggers = [(a[i], b[i])]))]
fn several_terms(a: &[i32], b: &[i32]) {
    prusti_assert!(a[1] == b[1]);
}
