//@ compile-flags: -Passert_timeout=45000
// Soundness canaries, part 4 (borrows and lending): every function must fail
// at its marked line.
use prusti_contracts::*;
use prusti_contracts::cell::{Cell, RefCell};
use prusti_contracts::rc::{Rc, Weak};

fn get_mut_then_use() {
    let mut c = RefCell::new(1);
    {
        let m = c.get_mut();
        *m = 5;
    }
    assert!(*c.borrow() == 5);
    assert!(false); //~ ERROR
}

fn deref_mut_then_try_borrow() {
    let c = RefCell::new(1);
    let mut g = c.borrow_mut();
    {
        let r: &mut i32 = &mut *g;
        *r = 2;
        assert!(c.try_borrow().is_err());
    }
    drop(g);
    assert!(*c.borrow() == 2);
    assert!(false); //~ ERROR
}

fn rc_get_mut_shared() {
    let mut a = Rc::new(1);
    let b = Rc::clone(&a);
    assert!(Rc::get_mut(&mut a).is_none());
    assert!(*b == 1);
    assert!(false); //~ ERROR
}

fn weak_after_drop() {
    let a = Rc::new(1);
    let w = Rc::downgrade(&a);
    drop(a);
    assert!(w.upgrade().is_none());
    assert!(false); //~ ERROR
}

fn cell_get_mut_lend() {
    let mut c = Cell::new(1);
    {
        let m = c.get_mut();
        *m = 3;
    }
    assert!(c.get() == 3);
    assert!(false); //~ ERROR
}

fn borrow_mut_while_ref_panics() {
    let c = RefCell::new(1);
    let r = c.borrow();
    let m = c.borrow_mut(); //~ ERROR
}
