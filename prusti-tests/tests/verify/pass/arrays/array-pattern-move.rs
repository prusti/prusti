//@ known-failure
use prusti_contracts::*;

// An array pattern moves an element out at a constant index. The PCG then
// holds every element of the array as a separate place, which the encoding
// does not: it unfolds an array at one index at a time. So collapsing the
// array once it holds no value is reported as unsupported.

pub struct S {
    pub v: i32,
}

fn consume(_s: S) {}

fn move_out_then_reassign() {
    let mut a = [S { v: 1 }, S { v: 2 }];
    let [x, _] = a;
    consume(x);
    a = [S { v: 3 }, S { v: 4 }];
    prusti_assert!(a[0].v == 3);
    prusti_refute!(false);
}
