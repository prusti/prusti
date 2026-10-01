//@ compile-flags: -Passert_timeout=45000
// Ending a `RefCell` guard's borrow must not make the state inconsistent:
// each function has to fail at its final assertion. (A guard reaches the
// cell's borrow flag through a reference, so a boundary with both the cell
// and a guard must grant the flag once, not once per holder.)
use std::cell::RefCell;

fn scope_end() {
    let c = RefCell::new(5);
    {
        let _r = c.borrow();
    }
    assert!(false); //~ERROR: the asserted expression might not hold
}

fn refmut_drop() {
    let c = RefCell::new(5);
    let m = c.borrow_mut();
    drop(m);
    assert!(false); //~ERROR: the asserted expression might not hold
}

fn two_borrows_live() {
    let c = RefCell::new(5);
    let _r1 = c.borrow();
    let _r2 = c.borrow();
    assert!(false); //~ERROR: the asserted expression might not hold
}

fn explicit_drop() {
    let c = RefCell::new(5);
    let r = c.borrow();
    drop(r);
    assert!(false); //~ERROR: the asserted expression might not hold
}

fn temporary_guard() {
    let c = RefCell::new(5);
    let _x = *c.borrow();
    assert!(false); //~ERROR: the asserted expression might not hold
}

fn drop_then_get_mut() {
    let mut c = RefCell::new(5);
    {
        let _r = c.borrow();
    }
    *c.get_mut() = 6;
    assert!(false); //~ERROR: the asserted expression might not hold
}

fn drop_then_replace() {
    let c = RefCell::new(5);
    {
        let _r = c.borrow();
    }
    c.replace(6);
    assert!(false); //~ERROR: the asserted expression might not hold
}
