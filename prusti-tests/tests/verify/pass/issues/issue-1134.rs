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

#[ensures(out.inner === out.inner)]
fn nested(out: Outer) {}

#[ensures(out.inner === out.inner)]
fn generic<T>(out: Generic<T>) {}

#[ensures(out.inner === old(out).inner)]
fn nested_old(out: Outer) {}

#[ensures(out.inner.0 == 5 ==> out.inner === Inner(5))]
fn nested_constructor(out: Outer) {}
