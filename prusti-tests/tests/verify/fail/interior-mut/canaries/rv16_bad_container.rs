//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub enum E { A(i32), B }
pub struct S<'a> { pub r: &'a i32, pub c: Cell<i32> }

#[pure]
#[trusted]
#[container_of(r, A)]
pub fn q<'a>(_s: &S<'a>) -> &'a E { unimplemented!() } //~ ERROR

pub fn client(s: &S) {
    prusti_assert!(q(s) === q(s));
}
