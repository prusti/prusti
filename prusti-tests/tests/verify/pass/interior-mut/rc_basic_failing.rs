//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// The `Rc` cases of `rc_basic.rs` Prusti does not verify yet:
// - two allocations are not known to be distinct (the provenance lemma for
//   live allocations is missing);
// - passing an `Rc` by value to a generic function (`into_inner`) returns its
//   allocation's atom with unknown contents.

use prusti_contracts::rc::Rc;

fn separate_allocations() {
    let a = Rc::new(1);
    let b = Rc::new(1);
    assert!(!Rc::ptr_eq(&a, &b));
    assert!(*a == *b);
}

// `into_inner` consumes its argument either way; only the last owner gets
// the value.
fn into_inner_last_owner() {
    let a = Rc::new(4);
    let b = Rc::clone(&a);
    assert!(Rc::into_inner(a).is_none());
    assert!(Rc::strong_count(&b) == 1);
    assert!(Rc::into_inner(b) == Some(4));
}

fn main() {
    separate_allocations();
    into_inner_last_owner();
}
