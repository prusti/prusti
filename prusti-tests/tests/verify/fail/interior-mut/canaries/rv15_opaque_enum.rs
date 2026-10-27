//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// Known failure: besides the expected errors, the type-level encodings of the
// unsupported struct layout fail without a position.
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

#[im_opaque]
pub struct S { o: Option<Cell<i32>> }

pub fn mk() -> S { //~ ERROR
    S { o: Some(Cell::new(1)) }
}

pub fn rd(x: &S) { //~ ERROR
    if let Some(c) = &x.o {
        c.set(4);
    }
}

pub fn mkf() { //~ ERROR
    let s = S { o: Some(Cell::new(1)) };
    assert!(false);
}
