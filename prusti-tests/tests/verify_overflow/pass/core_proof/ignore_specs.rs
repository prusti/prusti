//@ compile-flags: -Ponly_core_proof=true

use prusti_contracts::*;

fn test1() {
    assert!(false);
}

fn test2() {
    prusti_assert!(false);
}

#[requires(false)]
fn test3() { }

fn test4() {
    test3();
}

#[ensures(result == x)]
fn wrong_post(x: u32) -> u32 {
    x + 1
}

#[ensures(result > 0)]
fn unknown_result() -> u32 {
    0
}

fn unproven_post_of_callee() {
    let x = unknown_result();
    prusti_assert!(x == 42);
}

fn failing_refute() {
    prusti_refute!(true);
}

fn wrong_loop_invariant(n: u32) {
    let mut i = 0;
    while i < n {
        body_invariant!(i == 7);
        i += 1;
    }
}

#[requires(i < 3)]
fn ghost_code(i: usize) {
    ghost! {
        let s: Seq<u32> = seq![1, 2, 3];
        let _x = s[i];
    };
}

#[pure]
#[requires(i < 3)]
fn ghost_code_in_pure(i: usize) -> u32 {
    ghost! {
        let s: Seq<u32> = seq![1, 2, 3];
        let _x = s[i];
    };
    0
}

#[requires(Int::from(i) < s.len())]
fn seq_index(s: Seq<u32>, i: usize) -> Ghost<u32> {
    s[i]
}

fn explicit_panic(x: u32) -> u32 {
    if x == 0 {
        panic!("zero");
    }
    x
}

fn out_of_bounds(a: [u32; 3], i: usize) -> u32 {
    a[i]
}

fn overflow(x: u32, y: u32) -> u32 {
    x + y
}

fn division(x: u32, y: u32) -> u32 {
    x / y
}

// The ownership and borrowing of safe code is still verified.
fn moves_and_borrows(v: &mut Vec<u32>, x: Option<Box<u32>>) -> u32 {
    v.push(1);
    let r = &mut *v;
    r.push(2);
    match x {
        Some(b) => *b,
        None => v.len() as u32,
    }
}

// Panics are control flow: the paths where a call unwinds, running the
// cleanup code (here dropping `b`), are verified too.

fn id_mut(x: &mut u32) -> &mut u32 {
    x
}

fn unwinding_call_returning_reborrow(x: &mut u32, b: Box<u32>) {
    let r = id_mut(x);
    *r = *b;
}

fn read(x: &mut u32) -> u32 {
    *x
}

fn unwinding_call_dropping_box(x: &mut u32, b: Box<u32>) -> u32 {
    let r = read(&mut *x);
    r + *b
}

// `?` in a closure body, whose return value is only cast to the caller's
// generic context if the call returned.
fn try_in_closure() {
    let _f = |x: i32| -> Option<i32> { Some(Some(x)?) };
}
