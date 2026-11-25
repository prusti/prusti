//@ known-failure
use prusti_contracts::*;

// Moving out of a box keeps its allocation, which `*b = ..` then reuses. The
// PCG collapses `b` once its contents are moved out and expands it again for
// the assignment, which the encoding does not support.

struct S {
    v: i32,
}

fn consume(_s: S) {}

fn reinit_after_move() {
    let mut b = Box::new(S { v: 1 });
    let s = *b;
    *b = S { v: 5 };
    prusti_assert!(b.v == 5);
    consume(s);
}

fn cond_reinit(c: bool) {
    let mut b = Box::new(S { v: 1 });
    if c {
        let s = *b;
        consume(s);
        *b = S { v: 2 };
    }
    prusti_assert!(b.v == 1 || b.v == 2);
}
