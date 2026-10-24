// Every `Rc` operation must leave a consistent state: none of these
// functions may prove `false`.

use prusti_contracts::rc::{Rc, Weak};

fn new() {
    let _a = Rc::new(1);
    assert!(false); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn deref() {
    let a = Rc::new(1);
    let _x = *a;
    assert!(false); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn scope_drop() {
    {
        let _a = Rc::new(1);
    }
    assert!(false); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn mem_drop() {
    let a = Rc::new(1);
    drop(a);
    assert!(false); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn try_unwrap() {
    let a = Rc::new(1);
    let _ = Rc::try_unwrap(a);
    assert!(false); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn into_inner() {
    let a = Rc::new(1);
    let _ = Rc::into_inner(a);
    assert!(false); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn weak_new() {
    let w: Weak<i32> = Weak::new();
    let _ = w.upgrade();
    drop(w);
    assert!(false); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}

fn counts() {
    let a = Rc::new(1);
    let _n = Rc::strong_count(&a) + Rc::weak_count(&a);
    assert!(false); //~ERROR: precondition of `core::panicking::panic` (called by this macro expansion) might not hold
}
