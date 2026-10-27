//@ compile-flags: -Passert_timeout=45000
// Review round 9: a dependent atom (the `RefMut` pin) returned at its current
// amount, and generic values behind two mutable references.
use prusti_contracts::{
    cell::{Cell, RefCell, RefMut, refcell_count, refmut_cell, refmut_count},
    *,
};

#[requires(refmut_cell(&m) as *const RefCell<i32> === c as *const RefCell<i32>)]
#[requires(refmut_count(&m) == -1)]
fn drop_param(m: RefMut<i32>, c: &RefCell<i32>) {
    drop(m);
    let r = c.borrow();
    drop(r);
}

fn local(c: &RefCell<i32>) {
    if refcell_count(c) == 0 {
        let m = c.borrow_mut();
        drop(m);
        let r = c.borrow();
        drop(r);
        let m2 = c.borrow_mut();
        drop(m2);
    }
}

fn touch<T>(_x: &mut &mut T) {}

fn twice_mut() {
    let mut c = Cell::new(1);
    let mut r = &mut c;
    touch(&mut r); //~ ERROR
} //~ ERROR

fn plain<T>(_x: &mut T) {}

fn once_mut() {
    let mut c = Cell::new(1);
    plain(&mut c);
    let v = c.get();
    assert!(v == v);
}

fn ctrl() { assert!(false); } //~ ERROR
