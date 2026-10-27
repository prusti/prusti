//@ compile-flags: -Passert_timeout=45000
// A guard argument's `#[im_assume]` fact holds at the function's entry.
use prusti_contracts::*;
use prusti_contracts::cell::{RefCell, RefMut, Ref, ref_cell, refmut_cell};

#[requires(refmut_cell(&m) as *const RefCell<i32> === c as *const RefCell<i32>)]
pub fn refmut_param(m: RefMut<i32>, c: &RefCell<i32>) {
    drop(m);
    let r = c.borrow();
    drop(r);
}

#[requires(ref_cell(&r) as *const RefCell<i32> === c as *const RefCell<i32>)]
pub fn ref_param(r: Ref<i32>, c: &RefCell<i32>) {
    // The count may be at `isize::MAX` already.
    let r2 = c.borrow(); //~ ERROR
    drop(r2);
    drop(r);
}

#[requires(ref_cell(&r) as *const RefCell<i32> === c as *const RefCell<i32>)]
pub fn ref_param_mut(r: Ref<i32>, c: &RefCell<i32>) {
    let _m = c.borrow_mut(); //~ ERROR
    drop(r);
}

pub fn ctrl(m: RefMut<i32>) {
    drop(m);
    assert!(false); //~ ERROR
}
