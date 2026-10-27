//@ compile-flags: -Passert_timeout=45000
// Review round 3: `acc` preconditions at trait-stub calls; pure trait
// method results related to specs (addresses dummied); nested arrays of
// cells behind a field; unestablished `acc` postconditions; misplaced
// `acc`; raw derefs in `#[pure]` functions.
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub trait Reader {
    #[requires(acc(p, Real::WRITE))]
    #[ensures(acc(p, Real::WRITE))]
    unsafe fn read_at(&self, p: *const i32) -> i32;
}

pub fn generic_null<T: Reader>(t: &T) -> i32 {
    let p = core::ptr::null::<i32>();
    unsafe { t.read_at(p) } //~ ERROR
}

pub trait Tr {
    #[pure]
    fn val(&self) -> i32;
}

#[ensures(result == x.val())]
pub fn gen_val<T: Tr>(x: &T) -> i32 {
    x.val()
}

pub struct S {
    pub a: [[Cell<i32>; 2]; 2],
}

pub fn clobber(s: &S) { //~ ERROR
    s.a[0][0].set(9);
}

#[pure]
#[ensures(Real::NONE < result && result < Real::WRITE)]
fn half() -> Real {
    Real::WRITE / (Real::WRITE + Real::WRITE)
}

#[trusted]
#[requires(acc(p, half()))]
#[ensures(acc(p, half()))]
unsafe fn keep(p: *mut i32) {}

#[requires(acc(p, half()))]
#[ensures(acc(p, Real::WRITE))] //~ ERROR
unsafe fn grow(p: *mut i32) {
    unsafe { keep(p) }
}

#[trusted]
#[requires(!acc(p, Real::WRITE))] //~ ERROR
unsafe fn neg(p: *mut i32) {}

#[pure]
unsafe fn read(p: *const i32) -> i32 {
    unsafe { *p } //~ ERROR
    //~^ ERROR
}
