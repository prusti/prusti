//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// Known failure: besides the expected errors, the type-level encodings of the
// unsupported struct layout fail without a position.
use prusti_contracts::*;
use prusti_contracts::cell::{Cell, cell_value};

#[ptr_holds(ptr, Real::WRITE)]
pub struct G { ptr: *const i32, c: Cell<i32> }

pub fn rd(g: &G) -> i32 { //~ ERROR
    g.c.get()
}

#[requires(*cell_value(&g.c) == 3)]
pub fn spec(g: &G) { //~ ERROR
    let v = g.c.get();
    assert!(v == 3);
}

pub fn ctl(g: &G) { //~ ERROR
    let v = g.c.get();
    assert!(false);
}
