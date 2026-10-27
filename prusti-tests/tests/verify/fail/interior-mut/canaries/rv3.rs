//@ compile-flags: -Passert_timeout=45000
// Review round 1: `#[im_assume]` on an `#[im_opaque]` struct; `acc` in
// trait specs (axioms state values only; refinement of permissions).
use prusti_contracts::*;
use prusti_contracts::cell::UnsafeCell;

#[im_opaque]
#[im_perm(value, Real::WRITE)]
#[im_assume(true)]
pub struct MyCell {
    value: UnsafeCell<i32>,
}

fn pass(c: &MyCell) -> &MyCell {
    c
}

pub trait Reader {
    #[requires(acc(p, Real::WRITE))]
    #[ensures(acc(p, Real::WRITE))]
    unsafe fn read_at(&self, p: *const i32) -> i32;
}

pub struct S;

impl Reader for S {
    unsafe fn read_at(&self, p: *const i32) -> i32 {
        unsafe { *p }
    }
}

#[requires(acc(p, Real::WRITE))]
#[ensures(acc(p, Real::WRITE))]
pub fn generic<T: Reader>(t: &T, p: *const i32) -> i32 {
    // The trait stub states permissions through the pure axioms only, where
    // an `acc` precondition is `false` (review round 3): incomplete.
    unsafe { t.read_at(p) } //~ ERROR
}

pub trait Plain {
    unsafe fn get_at(&self, p: *const i32) -> i32;
}

#[refine_trait_spec]
impl Plain for S {
    #[trusted]
    #[requires(acc(p, Real::WRITE))] //~ ERROR
    #[ensures(acc(p, Real::WRITE))]
    unsafe fn get_at(&self, p: *const i32) -> i32 {
        unsafe { *p }
    }
}
