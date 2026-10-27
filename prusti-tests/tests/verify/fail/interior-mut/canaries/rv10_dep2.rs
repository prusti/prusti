//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use core::cell::UnsafeCell;

#[im_opaque]
#[im_dependent(a, Real::WRITE)]
#[im_dependent(b, Real::WRITE)]
pub struct Two { //~ ERROR
    a: UnsafeCell<i32>,
    b: UnsafeCell<i32>,
}

pub fn use_two(t: &Two) -> bool {
    let _ = t;
    true
}

pub fn ctrl() { assert!(false); } //~ ERROR
