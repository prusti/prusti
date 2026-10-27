//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// The `Weak` case of `rc_weak.rs` Prusti does not verify yet: a parent with
// interior-mutable state inside an `Rc` (not supported yet).

use prusti_contracts::{
    cell::Cell,
    rc::{Rc, Weak},
};

struct Parent {
    value: Cell<i32>,
}

struct Child {
    parent: Weak<Parent>,
}

// A child writes to its parent through a weak back-reference.
fn child_writes_to_parent() {
    let p = Rc::new(Parent {
        value: Cell::new(0),
    });
    let c = Child {
        parent: Rc::downgrade(&p),
    };
    match c.parent.upgrade() {
        Some(pp) => pp.value.set(1),
        None => unreachable!(),
    }
    assert!(p.value.get() == 1);
    assert!(Rc::strong_count(&p) == 1);
    drop(p);
    assert!(c.parent.upgrade().is_none());
}

fn main() {
    child_writes_to_parent();
}
