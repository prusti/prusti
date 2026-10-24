//@ compile-flags: -Passert_timeout=45000
// Writing through a `RefMut` leaves the borrow flag alone, so the cell is
// free again once the guard is dropped.
use prusti_contracts::cell::RefCell;

fn borrow_mut_write_then_borrow() {
    let c = RefCell::new(0);
    {
        let mut m = c.borrow_mut();
        *m = 7;
    }
    assert!(*c.borrow() == 7);
}
