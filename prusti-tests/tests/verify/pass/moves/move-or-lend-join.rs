//@ known-failure
use prusti_contracts::*;

// A place moved out on one path and lent out on another holds a value after
// the join only where it was lent out. The PCG does not track this yet, so
// these tests are rejected with an internal error.

pub struct S {
    pub v: i32,
}

fn consume(_s: S) {}

// Each loop exit moves one of `a` and `b` and lends out the other. Assigning
// both after the loop must replace the value that is still held: adding a
// second predicate for the same place would make the path infeasible, which
// the refutation detects.
fn exits_move_or_lend(c: bool, d: bool) {
    let mut a = S { v: 1 };
    let mut b = S { v: 2 };
    let mut i = 0;
    let r;
    loop {
        if c {
            r = &mut a;
            consume(b);
            break;
        }
        if d {
            r = &mut b;
            consume(a);
            break;
        }
        body_invariant!(i < 100);
        if i >= 99 {
            return;
        }
        i += 1;
    }
    r.v = 7;
    a = S { v: 3 };
    b = S { v: 4 };
    prusti_assert!(a.v == 3 && b.v == 4);
    prusti_refute!(false);
}

pub struct Pair {
    pub x: S,
    pub y: S,
}

fn read(_s: &i32) {}

// `p.x` is moved out on one branch and lent out (shared) on the others. When
// the loans expire, `p.x.v` holds a value only on the lending branches, where
// the value is dropped before the reassignment.
fn move_or_share(c: u8) {
    let mut p = Pair { x: S { v: 1 }, y: S { v: 2 } };
    let o = 0;
    let r;
    if c == 0 {
        consume(p.x);
        r = &o;
    } else if c == 1 {
        r = &p.x.v;
    } else {
        r = &p.x.v;
    }
    read(r);
    p.x = S { v: 5 };
    assert!(p.x.v == 5);
    prusti_refute!(false);
}
