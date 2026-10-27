//@ compile-flags: -Passert_timeout=45000
// Review round 12: lent footprints with several atoms in wands, folded
// dependent atoms in an `#[im_opaque]` struct with amounts, sibling fields
// behind one shared reference.
use prusti_contracts::*;
use prusti_contracts::cell::{Cell, RefCell};
use core::cell::UnsafeCell;

pub struct P { pub a: Cell<i32>, pub b: Cell<i32>, pub n: i32 }

pub fn num(p: &mut P) -> &mut i32 { &mut p.n }

pub fn h(p: &mut P) -> &mut Cell<i32> { &mut p.a }

pub fn wand_caller() {
    let mut p = P { a: Cell::new(1), b: Cell::new(2), n: 3 };
    let r = num(&mut p);
    *r = 5;
    let c = h(&mut p);
    c.set(4);
    let _ = p.b.get();
}

pub fn siblings(p: &P) {
    let x = p.a.get();
    p.b.set(3);
    assert!(p.a.get() == x);
    assert!(p.b.get() == 3);
}

pub fn tuple_siblings(p: &(Cell<i32>, Cell<i32>)) {
    let x = p.0.get();
    p.1.set(3);
    assert!(p.0.get() == x);
}

pub fn aliased(p: &P, q: &P) {
    p.a.set(1);
    q.a.set(2);
    assert!(p.a.get() == 1); //~ ERROR
}

#[im_opaque]
#[im_perm(v, Real::WRITE)]
pub struct S { r: RefCell<i32>, v: UnsafeCell<i32> }

pub fn opaque_dependent() {
    let s = S { r: RefCell::new(1), v: UnsafeCell::new(2) };
    let b = s.r.borrow_mut();
    drop(b);
}

pub fn ctrl() { assert!(false); } //~ ERROR
