//@ compile-flags: -Passert_timeout=45000
// Soundness canaries, part 3: every function must fail at its marked line.
use prusti_contracts::*;
use prusti_contracts::cell::{Cell, RefCell};

struct S {
    c: Cell<i32>,
    d: i32,
}

fn overwrite_field() {
    let mut s = S { c: Cell::new(1), d: 0 };
    s.c = Cell::new(2);
    assert!(s.c.get() == 2);
    assert!(false); //~ ERROR
}

fn overwrite_option() {
    let mut o = Some(Cell::new(1));
    o = Some(Cell::new(2));
    if let Some(c) = &o {
        assert!(c.get() == 2);
    }
    assert!(false); //~ ERROR
}

fn replace_behind_mut() {
    let mut a = Cell::new(1);
    let old = core::mem::replace(&mut a, Cell::new(2));
    assert!(false); //~ ERROR
}

fn take_cell_by_value(c: Cell<i32>) -> Cell<i32> {
    c.set(c.get() + 1);
    c
}

fn round_trip_by_value() {
    let c = Cell::new(1);
    let c = take_cell_by_value(c);
    assert!(false); //~ ERROR
}

fn forget_guard() {
    let c = RefCell::new(1);
    let r = c.borrow();
    core::mem::forget(r);
    assert!(c.try_borrow_mut().is_err());
    assert!(false); //~ ERROR
}
