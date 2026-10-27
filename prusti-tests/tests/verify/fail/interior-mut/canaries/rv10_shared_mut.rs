//@ compile-flags: -Passert_timeout=45000
// Review round 6: atoms behind a `&mut` reached through a shared reference
// may alias (shared).
use prusti_contracts::cell::Cell;

pub struct Wrapper<'a> {
    pub c: &'a mut Cell<i32>,
}

pub fn f(_a: &Wrapper, _b: &Wrapper) {}

pub fn caller() {
    let mut c = Cell::new(0);
    let w = Wrapper { c: &mut c };
    f(&w, &w);
}

pub fn g(_a: &&mut Cell<i32>, _b: &&mut Cell<i32>) {}

pub fn caller_ref() {
    let mut c = Cell::new(0);
    let r = &mut c;
    g(&r, &r);
}

pub fn ctrl() { assert!(false); } //~ ERROR
