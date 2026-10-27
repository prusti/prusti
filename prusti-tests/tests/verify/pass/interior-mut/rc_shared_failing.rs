//@ known-failure
//@ compile-flags: -Passert_timeout=45000
// Shared mutable state through `Rc<Cell<_>>`: writes through one clone are
// visible through all others, and functions taking several `Rc`s (by
// reference or by value) must only assert facts that also hold when they
// share an allocation. Interior-mutable values inside an `Rc` are not
// supported yet.

use prusti_contracts::{cell::Cell, rc::Rc, *};

fn write_through_clone_visible() {
    let a = Rc::new(Cell::new(0));
    let b = Rc::clone(&a);
    b.set(1);
    assert!(a.get() == 1);
    a.set(2);
    assert!(b.get() == 2);
}

fn distinct_allocations_independent() {
    let a = Rc::new(Cell::new(0));
    let b = Rc::new(Cell::new(0));
    a.set(1);
    assert!(b.get() == 0);
}

// `a` and `b` may share an allocation, so after both writes only the second
// is known.
fn last_write_wins(a: &Rc<Cell<i32>>, b: &Rc<Cell<i32>>) {
    a.set(1);
    b.set(2);
    assert!(b.get() == 2);
}

// Owning two `Rc`s does not make them disjoint either.
fn owned_rcs_may_alias(a: Rc<Cell<i32>>, b: Rc<Cell<i32>>) {
    a.set(1);
    b.set(2);
    assert!(b.get() == 2);
}

fn ptr_eq_means_shared(a: &Rc<Cell<i32>>, b: &Rc<Cell<i32>>) {
    if Rc::ptr_eq(a, b) {
        a.set(3);
        assert!(b.get() == 3);
    }
}

// Cloning and dropping `Rc`s changes only the reference counts, never the
// shared value.
fn clone_and_drop_preserve_value(a: &Rc<Cell<i32>>) {
    a.set(4);
    let b = Rc::clone(a);
    drop(b);
    assert!(a.get() == 4);
}

fn callers() {
    let c = Rc::new(Cell::new(0));
    let d = Rc::new(Cell::new(9));
    last_write_wins(&c, &c);
    last_write_wins(&c, &d);
    owned_rcs_may_alias(Rc::clone(&c), Rc::clone(&c));
    owned_rcs_may_alias(Rc::clone(&c), Rc::clone(&d));
    ptr_eq_means_shared(&c, &c);
    ptr_eq_means_shared(&c, &d);
    clone_and_drop_preserve_value(&c);
    assert!(c.get() == 4);
    assert!(Rc::strong_count(&c) == 1);
}

// A clone passed away by value keeps writing to the same cell.
fn write_via_moved_clone() {
    let a = Rc::new(Cell::new(0));
    let b = Rc::clone(&a);
    let moved = b;
    moved.set(7);
    assert!(a.get() == 7);
}

fn loop_with_invariant() {
    let a = Rc::new(Cell::new(0));
    let b = Rc::clone(&a);
    let mut i = 0;
    while i < 10 {
        body_invariant!(0 <= i && i <= 10);
        body_invariant!(a.get() == i);
        b.set(b.get() + 1);
        i += 1;
    }
    assert!(a.get() == 10);
    assert!(b.get() == 10);
}

struct Holder {
    counter: Rc<Cell<u32>>,
}

// Two structs sharing one counter.
fn shared_counter_between_holders() {
    let c = Rc::new(Cell::new(0));
    let h1 = Holder {
        counter: Rc::clone(&c),
    };
    let h2 = Holder {
        counter: Rc::clone(&c),
    };
    h1.counter.set(h1.counter.get() + 1);
    h2.counter.set(h2.counter.get() + 1);
    assert!(c.get() == 2);
    assert!(h1.counter.get() == 2);
    assert!(Rc::strong_count(&c) == 3);
    drop(h1);
    assert!(Rc::strong_count(&c) == 2);
    assert!(h2.counter.get() == 2);
}

fn main() {
    write_through_clone_visible();
    distinct_allocations_independent();
    callers();
    write_via_moved_clone();
    loop_with_invariant();
    shared_counter_between_holders();
}
