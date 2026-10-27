//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub struct S { pub x: u32, pub c: Cell<i32> }

#[pure]
#[trusted]
#[pointee_of(x)]
pub fn p(_s: &S) -> &i32 { unimplemented!() } //~ ERROR

pub fn client(s: &S) {
    prusti_assert!(*p(s) == *p(s));
}
