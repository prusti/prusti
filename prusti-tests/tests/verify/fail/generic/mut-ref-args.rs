//@ known-failure
use prusti_contracts::*;

// A generic function may change the target of a mutable reference that its
// caller passes as (part of) a type-parameter argument. Its contract does not
// take that target yet, so the caller wrongly keeps the old value.

trait Set {
    fn set(&mut self);
}

impl Set for &mut i32 {
    fn set(&mut self) {
        **self = 1;
    }
}

fn set<T: Set>(mut t: T) {
    t.set();
}

fn reference() {
    let mut c = 0;
    set(&mut c);
    assert!(c == 0); //~ERROR: might not hold
}

fn apply_twice<F: FnMut()>(mut f: F) {
    f();
    f();
}

fn closure() {
    let mut c = 0;
    apply_twice(|| c += 1);
    assert!(c == 0); //~ERROR: might not hold
}
