//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub struct W<T>(pub T);
impl<T> Drop for W<T> { fn drop(&mut self) {} }

#[extern_spec]
impl Drop for W<i32> {
    #[ensures(false)] //~ ERROR
    fn drop(&mut self);
}

pub fn drop_u8() {
    let _c = Cell::new(0);
    {
        let _w = W(1u8);
    }
    assert!(false);
}

pub fn drop_i32() {
    let _c = Cell::new(0);
    {
        let _w = W(1i32);
    }
    assert!(false);
}

pub fn ctrl() { assert!(false); }
