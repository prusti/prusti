//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use core::cell::UnsafeCell;

pub struct Foo;

impl Foo {
    pub fn new() -> UnsafeCell<u8> {
        UnsafeCell::new(0)
    }
}

pub fn control() {
    assert!(false); //~ ERROR
}
