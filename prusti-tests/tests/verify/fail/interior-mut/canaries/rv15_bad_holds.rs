//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

#[ptr_holds(pt, Real::WRITE)]
pub struct G { ptr: *const i32 } //~ ERROR

pub fn f(g: &G, c: &Cell<i32>) -> i32 { c.get() }
