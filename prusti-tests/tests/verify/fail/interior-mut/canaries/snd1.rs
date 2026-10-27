//@ compile-flags: -Passert_timeout=45000
// Soundness canaries: every function must fail at its `assert!(false)`.
use prusti_contracts::*;
use prusti_contracts::cell::{Cell, RefCell};
use prusti_contracts::rc::Rc;

fn reassign_cell() {
    let mut c = Cell::new(1);
    c = Cell::new(2);
    assert!(c.get() == 2);
    assert!(false); //~ ERROR
}

fn reassign_cell_default() {
    let mut c: Cell<i32> = Cell::new(1);
    c = Default::default();
    assert!(c.get() == 1); //~ ERROR
}

fn reassign_refcell_while_borrowed_elsewhere() {
    let mut c = RefCell::new(1);
    c = RefCell::new(2);
    let r = c.borrow();
    assert!(*r == 2);
    drop(r);
    assert!(false); //~ ERROR
}

fn two_refs_then_mut() {
    let c = RefCell::new(1);
    let r1 = c.borrow();
    let r2 = c.borrow();
    drop(r1);
    drop(r2);
    let m = c.borrow_mut();
    drop(m);
    assert!(false); //~ ERROR
}

fn from_mut_after_borrow() {
    let mut x = 1;
    {
        let c = Cell::from_mut(&mut x);
        c.set(2);
    }
    x = 3;
    assert!(x == 3);
    assert!(false); //~ ERROR
}

fn swap_cells() {
    let mut a = Cell::new(1);
    let mut b = Cell::new(2);
    core::mem::swap(&mut a, &mut b);
    assert!(false); //~ ERROR
}

fn refcell_self_swap() {
    let c = RefCell::new(1);
    c.swap(&c); //~ ERROR
}

fn rc_clone_drop() {
    let a = Rc::new(1);
    let b = Rc::clone(&a);
    drop(a);
    let c = Rc::clone(&b);
    drop(b);
    assert!(Rc::strong_count(&c) == 1);
    assert!(false); //~ ERROR
}

fn cell_in_struct_moved() {
    struct S {
        c: Cell<i32>,
    }
    let s = S { c: Cell::new(1) };
    let t = s;
    t.c.set(5);
    let u = S { c: Cell::new(7) };
    assert!(t.c.get() == 5 && u.c.get() == 7);
    assert!(false); //~ ERROR
}
