//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub fn g<'a>(x: &'a mut i32) -> (&'a mut i32, Cell<i32>) {
    (x, Cell::new(3))
}

pub struct W<'a> { pub r: &'a mut i32, pub c: Cell<i32> }

pub fn h<'a>(w: W<'a>) -> &'a mut i32 {
    w.r
}

pub fn client_tuple() {
    let mut v = 1;
    let t = g(&mut v);
    *t.0 = 7;
    v = 3;
}

pub fn client_arg() {
    let mut v = 1;
    let r = h(W { r: &mut v, c: Cell::new(1) });
    *r = 7;
    v = 3;
}

pub fn ctl() {
    assert!(false); //~ ERROR
}
