//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::RefCell;

pub struct W<'a>(prusti_contracts::cell::RefMut<'a, i32>, u8);

pub fn st() {
    let c = RefCell::new(0);
    {
        let _p = W(c.borrow_mut(), 1);
    }
    let _g = c.borrow_mut();
}

pub fn tup() {
    let c = RefCell::new(0);
    {
        let _p = (c.borrow_mut(), 1u8);
    }
    let _g = c.borrow_mut();
}

pub fn control() {
    let c = RefCell::new(0);
    assert!(false); //~ ERROR
}
