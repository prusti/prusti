//@ known-failure
use prusti_contracts::*;

// A box unpacked at a loop head. The PCG unpacks a box `b` into `*b` only,
// but its encoding also holds the box's `Unique` pointer and allocator
// fields, which the address of `*b` and dropping the box need. The loop
// invariant holds only the places the PCG reports, so it lacks these fields.
// This should pass once the PCG unpacks a box into `*b`, `b.0` and `b.1`.

pub struct S {
    pub v: u32,
}

fn consume(_s: S) {}

fn reassign_in_loop(n: u32) {
    let mut b = Box::new(S { v: 1 });
    consume(*b);
    let mut i = 0;
    while i < n {
        body_invariant!(i < n);
        b = Box::new(S { v: i });
        i += 1;
    }
    prusti_refute!(false);
}

fn read_in_loop(n: u32) -> u32 {
    let b = Box::new(S { v: 1 });
    let mut acc = 0;
    let mut i = 0;
    while i < n {
        body_invariant!(i < n);
        acc = b.v;
        i += 1;
    }
    prusti_assert!(n == 0 || acc == 1);
    acc
}
