//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// The `RefCell` cases of `refcell_types.rs` Prusti does not verify yet:
// - interior-mutable values inside a `RefCell` (not supported yet);
// - `==` on the contents of a generic or primitive type, which goes through
//   the `PartialEq` trait (whose calls have no semantics for foreign impls,
//   and which is not reflexive in general);
// - the contents of a clone (`T::clone` has no generic meaning).

use prusti_contracts::{
    cell::{Cell, RefCell, refcell_count},
    *,
};

// A `RefCell` behind a shared reference may be borrowed by others: the caller
// has to promise it is free.
#[requires(refcell_count(c) == 0)]
fn generic_write_read<T: Copy + PartialEq>(c: &RefCell<T>, v: T) {
    {
        let mut m = c.borrow_mut();
        *m = v;
    }
    assert!(*c.borrow() == v);
}

fn generic_callers() {
    let a = RefCell::new(0u8);
    generic_write_read(&a, 5);
    let b = RefCell::new(false);
    generic_write_read(&b, true);
}

fn nested_refcell() {
    let c = RefCell::new(RefCell::new(1));
    {
        let outer = c.borrow();
        let mut inner = outer.borrow_mut();
        *inner = 2;
    }
    assert!(*c.borrow().borrow() == 2);
}

fn cell_inside_refcell() {
    let c = RefCell::new(Cell::new(1));
    {
        // Only a shared borrow is needed to mutate the inner `Cell`.
        let r = c.borrow();
        r.set(2);
    }
    assert!(c.borrow().get() == 2);
}

fn clone_is_independent() {
    let a = RefCell::new(1);
    let b = a.clone();
    *a.borrow_mut() = 5;
    assert!(*a.borrow() == 5);
    assert!(*b.borrow() == 1);
}

fn comparisons_use_contents() {
    let a = RefCell::new(1);
    let b = RefCell::new(1);
    assert!(a == b);
    b.replace(2);
    assert!(a != b);
    assert!(a < b);
    assert!(b > a);
}

fn main() {
    generic_callers();
    nested_refcell();
    cell_inside_refcell();
    clone_is_independent();
    comparisons_use_contents();
}
