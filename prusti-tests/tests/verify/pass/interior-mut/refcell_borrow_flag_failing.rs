//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// The `RefCell` borrow-flag case of `refcell_borrow_flag.rs` Prusti does not
// verify yet: writing through a `RefMut` inside a loop (a gap in the loop
// encoding of writes through mutable references, not specific to `RefCell`).

use prusti_contracts::{
    cell::{RefCell, refcell_count},
    *,
};

fn interleaved_borrow_cycles() {
    let c = RefCell::new(0);
    let mut i = 0;
    while i < 3 {
        body_invariant!(0 <= i && i <= 3);
        // Each iteration takes and fully releases an exclusive borrow, so the
        // next iteration's `borrow_mut` cannot panic. The borrow state is
        // interior-mutable, so its preservation is part of the invariant.
        body_invariant!(refcell_count(&c) == 0);
        let mut m = c.borrow_mut();
        *m += 1;
        drop(m);
        let r = c.borrow();
        let _v = *r;
        drop(r);
        i += 1;
    }
    assert!(c.try_borrow_mut().is_ok());
}

fn main() {
    interleaved_borrow_cycles();
}
