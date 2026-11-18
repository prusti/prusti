// Regression test for prusti/prusti#1134: snapshot equality of nested struct fields.
use prusti_contracts::*;

#[derive(Clone, Copy)]
struct Inner(u32);

#[derive(Clone, Copy)]
struct Outer {
    inner: Inner,
}

#[derive(Clone, Copy)]
struct Generic<T> {
    inner: T,
}

#[ensures(!(out.inner === out.inner))] //~ ERROR: postcondition might not hold
fn nested_negated(out: Outer) {}

#[ensures(!(out.inner === out.inner))] //~ ERROR: postcondition might not hold
fn generic_negated<T>(out: Generic<T>) {}

#[ensures(a.inner === b.inner)] //~ ERROR: postcondition might not hold
fn different_values(a: Outer, b: Outer) {}

#[ensures(out.inner === Inner(5))] //~ ERROR: postcondition might not hold
fn wrong_constructor(out: Outer) {}
