//@ compile-flags: -Passert_timeout=45000
// Review round 1: drop contracts of the held parts of a partially moved
// value.
use prusti_contracts::cell::{Ref, RefCell};

pub struct N(u32);
pub struct S<'a> {
    g: Ref<'a, i32>,
    n: N,
}

fn take(_n: N) {}

pub fn partial_move_then_scope_end() {
    let c = RefCell::new(5);
    {
        let s = S { g: c.borrow(), n: N(1) };
        take(s.n);
    }
    let _m = c.borrow_mut();
}

pub fn still_borrowed() {
    let c = RefCell::new(5);
    let s = S { g: c.borrow(), n: N(1) };
    take(s.n);
    let _m = c.borrow_mut(); //~ ERROR
    drop(s.g);
}
