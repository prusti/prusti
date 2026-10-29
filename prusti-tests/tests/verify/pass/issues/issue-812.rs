use prusti_contracts::*;

#[requires(forall(|i: usize| (0 <= i && i < slice.len()) ==> (slice[i] > 10), triggers=[(slice[i],)]))]
fn test_slice(slice: &[i32]) {}

#[requires(forall(|i: usize| (0 <= i && i < array.len()) ==> (array[i] > 10), triggers=[(array[i],)]))]
fn test_array(array: &[i32; 10]) {}

#[requires(forall(|i: usize| (0 <= i && i < slice.len()) ==> (bar(slice[i])), triggers=[(bar(slice[i]),)]))]
//#[requires(forall(|i: usize| (0 <= i && i < slice.len()) ==> (bar(slice[i])), triggers=[(bar(slice[i]),(slice.len()),)]))] “(slice.len())” cannot be used as a trigger due to https://github.com/viperproject/silver/issues/617
fn foo(slice: &[i32]) {}

#[pure]
#[trusted]
fn bar(a: i32) -> bool { true }

#[requires(slice.len() == 1)]
fn baz(slice: &[i32]) {
    if bar(slice[0]) {
        foo(slice);
    }
}

// Triggers with several terms.

#[pure]
#[requires(forall(|i: usize| (0 <= i && i < slice.len()) ==> (slice[i] > 10), triggers=[(slice[i],slice.len(),baz_multi(slice))]))]
fn foo_multi(slice: &[i32]) {}

#[requires(forall(|i: usize| (0 <= i && i < array.len()) ==> (array[i] > 10), triggers=[(array[i],array.len(),baz_multi(array),)]))]
fn bar_multi(array: &[i32; 10]) {}

#[pure]
#[trusted]
fn baz_multi(slice: &[i32]) {}
