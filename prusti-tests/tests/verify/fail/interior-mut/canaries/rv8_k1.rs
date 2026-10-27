//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// Known failure: the generic associated constant gives an error without a
// position.
use prusti_contracts::*;

pub trait Tr { const C: u32 = 0; }
pub struct S;
impl Tr for S { const C: u32 = 1; }

pub fn f<T: Tr>() {
    assert!(T::C == 0); //~ ERROR
}

pub fn g() {
    f::<S>();
}

pub fn control() {
    assert!(false); //~ ERROR
}
