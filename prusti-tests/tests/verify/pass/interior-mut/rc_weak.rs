//@ compile-flags: -Passert_timeout=45000
// `std::rc::Weak`: downgrading, upgrading and the weak count. Whether
// `upgrade` succeeds depends on the strong count, which is interior-mutable
// state shared with every `Rc` of the allocation. The cases Prusti does not
// verify yet are in `rc_weak_failing.rs`.

use prusti_contracts::rc::{Rc, Weak};

fn upgrade_while_alive() {
    let a = Rc::new(5);
    let w = Rc::downgrade(&a);
    assert!(Rc::weak_count(&a) == 1);
    match w.upgrade() {
        Some(b) => {
            assert!(*b == 5);
            assert!(Rc::ptr_eq(&a, &b));
            assert!(Rc::strong_count(&a) == 2);
        }
        None => unreachable!(),
    }
    assert!(Rc::strong_count(&a) == 1);
}

fn upgrade_after_last_drop() {
    let a = Rc::new(5);
    let b = Rc::clone(&a);
    let w = Rc::downgrade(&a);
    drop(a);
    // `b` still keeps the allocation alive.
    assert!(w.upgrade().is_some());
    drop(b);
    assert!(w.upgrade().is_none());
    assert!(w.strong_count() == 0);
    assert!(w.weak_count() == 0);
}

fn weak_new_never_upgrades() {
    let w: Weak<i32> = Weak::new();
    assert!(w.upgrade().is_none());
    assert!(w.strong_count() == 0);
    assert!(w.weak_count() == 0);
}

fn weak_count_tracks_downgrades() {
    let a = Rc::new(1);
    let w1 = Rc::downgrade(&a);
    let w2 = w1.clone();
    assert!(Rc::weak_count(&a) == 2);
    assert!(w1.weak_count() == 2);
    assert!(w1.ptr_eq(&w2));
    drop(w2);
    assert!(Rc::weak_count(&a) == 1);
    assert!(w1.strong_count() == 1);
}

fn main() {
    upgrade_while_alive();
    upgrade_after_last_drop();
    weak_new_never_upgrades();
    weak_count_tracks_downgrades();
}
