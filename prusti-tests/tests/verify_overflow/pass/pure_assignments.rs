//! Assignments to fields of local variables in pure code.
use prusti_contracts::*;

#[pure]
fn pure1() -> (u32, u32) {
    let mut x = (1, 2);
    x.1 = 3;
    x
}

fn client1() {
    let x = pure1();
    assert!(x == (1, 3));
}

#[pure]
fn nested() -> ((u32, u32), u32) {
    let mut x = ((1, 2), 3);
    x.0.1 = 4;
    x.1 = 5;
    x
}

fn client_nested() {
    let x = nested();
    assert!(x.0.0 == 1 && x.0.1 == 4 && x.1 == 5);
}

#[pure]
fn branch(c: bool) -> (u32, u32) {
    let mut x = (0, 0);
    if c {
        x.0 = 1;
    } else {
        x.1 = 2;
    }
    x
}

fn client_branch() {
    assert!(branch(true) == (1, 0));
    assert!(branch(false) == (0, 2));
}

#[pure]
fn from_call() -> (u32, u32) {
    let mut x = (0, 0);
    x.1 = pure1().1;
    x
}

fn client_from_call() {
    assert!(from_call() == (0, 3));
}

struct S {
    a: u32,
    b: (u32, bool),
}

#[pure]
#[requires(v < 100)]
fn update_arg(mut s: S, v: u32) -> u32 {
    s.b.0 = v;
    s.a = s.b.0 + 1;
    s.a
}

#[requires(v < 100)]
#[ensures(result == v + 1)]
fn client_update_arg(v: u32) -> u32 {
    update_arg(S { a: 0, b: (0, true) }, v)
}

#[derive(Clone, Copy)]
struct W<T> {
    val: T,
    n: u32,
}

#[pure]
fn set_val<T: Copy>(mut w: W<T>, v: T) -> W<T> {
    w.val = v;
    w
}

fn client_generic() {
    let w = set_val(W { val: true, n: 3 }, false);
    assert!(w.n == 3 && !w.val);
}

#[pure]
fn set_box(mut b: Box<(u32, u32)>) -> u32 {
    (*b).1 = 7;
    *b = (b.1, 8);
    b.0 + b.1
}

fn client_box() {
    assert!(set_box(Box::new((1, 2))) == 15);
}
