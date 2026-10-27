//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;

#[requires(acc(p, Real::WRITE))]
#[requires(unsafe { *p } == 5)]
#[ensures(acc(p, Real::WRITE))]
#[ensures(old(unsafe { *p }) == 6)] //~ ERROR
pub unsafe fn w(p: *mut i32) {
    unsafe { *p = 6; }
}

#[requires(acc(p, Real::WRITE))]
#[requires(unsafe { *p } == 5)]
#[ensures(acc(p, Real::WRITE))]
#[ensures(old(unsafe { *p }) == 5)]
pub unsafe fn w_ok(p: *mut i32) {
    unsafe { *p = 6; }
}

#[requires(acc(p, Real::WRITE))]
#[ensures(acc(p, Real::WRITE))]
pub unsafe fn control(p: *mut i32) {
    assert!(false); //~ ERROR
}
