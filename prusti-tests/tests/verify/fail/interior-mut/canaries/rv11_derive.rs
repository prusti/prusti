//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

#[derive(PartialEq)]
pub struct S { pub c: Cell<i32> }

pub fn derived_eq_frozen() {
    let s1 = S { c: Cell::new(0) };
    let s2 = S { c: Cell::new(0) };
    let e1 = s1 == s2;
    s1.c.set(1);
    let e2 = s1 == s2;
    assert!(e1 == e2); //~ ERROR
}

pub fn control() {
    let s1 = S { c: Cell::new(0) };
    assert!(false); //~ ERROR
}
