//@ compile-flags: -Passert_timeout=45000
// Basic `std::rc::Rc` API: construction, dereferencing, cloning, the strong
// count and unique access. The strong count lives in a `Cell` shared by all
// clones, so every clone and drop mutates state reachable from the others.
// The cases Prusti does not verify yet are in `rc_basic_failing.rs`.

use prusti_contracts::rc::Rc;

fn new_and_deref() {
    let r = Rc::new(5);
    assert!(*r == 5);
}

fn clone_shares_allocation() {
    let a = Rc::new(5);
    let b = Rc::clone(&a);
    assert!(Rc::ptr_eq(&a, &b));
    assert!(*a == 5);
    assert!(*b == 5);
}

fn strong_count_tracks_clones() {
    let a = Rc::new(1);
    assert!(Rc::strong_count(&a) == 1);
    let b = a.clone();
    assert!(Rc::strong_count(&a) == 2);
    assert!(Rc::strong_count(&b) == 2);
    drop(b);
    assert!(Rc::strong_count(&a) == 1);
}

// Cloning through an alias of `a` changes the count observed through `a`.
fn strong_count_through_alias() {
    let a = Rc::new(1);
    let r = &a;
    let b = Rc::clone(r);
    assert!(Rc::strong_count(&a) == 2);
    drop(b);
    assert!(Rc::strong_count(r) == 1);
}

fn get_mut_when_unique() {
    let mut a = Rc::new(1);
    match Rc::get_mut(&mut a) {
        Some(v) => *v = 2,
        None => unreachable!(),
    }
    assert!(*a == 2);
}

fn get_mut_when_shared() {
    let mut a = Rc::new(1);
    let b = Rc::clone(&a);
    assert!(Rc::get_mut(&mut a).is_none());
    drop(b);
    assert!(Rc::get_mut(&mut a).is_some());
}

fn try_unwrap_unique() {
    let a = Rc::new(3);
    match Rc::try_unwrap(a) {
        Ok(v) => assert!(v == 3),
        Err(_) => unreachable!(),
    }
}

fn try_unwrap_shared_returns_rc() {
    let a = Rc::new(3);
    let b = Rc::clone(&a);
    match Rc::try_unwrap(a) {
        Ok(_) => unreachable!(),
        Err(a) => {
            assert!(Rc::ptr_eq(&a, &b));
            assert!(Rc::strong_count(&b) == 2);
        }
    }
}

fn main() {
    new_and_deref();
    clone_shares_allocation();
    strong_count_tracks_clones();
    strong_count_through_alias();
    get_mut_when_unique();
    get_mut_when_shared();
    try_unwrap_unique();
    try_unwrap_shared_returns_rc();
}
