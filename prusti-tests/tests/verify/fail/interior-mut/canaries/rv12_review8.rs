//@ compile-flags: -Passert_timeout=45000
// Review round 8: `#[pointee_of]`/`#[container_of]` values, shared atoms
// inside the referent of a `&mut` argument, `ManuallyDrop` contents.
#![allow(undropped_manually_drops, dead_code)]
use prusti_contracts::*;
use prusti_contracts::cell::Cell;
use prusti_contracts::rc::Rc;
use std::mem::ManuallyDrop;

pub struct Node { pub val: i32 }
pub struct Holder { p: *const Node }

#[pure]
#[trusted]
#[pointee_of(p)]
pub fn node(_h: &Holder) -> &Node { unimplemented!() }

#[requires(node(h1).val == 1)]
pub fn pointees(h1: &Holder, h2: &Holder) {
    assert!(node(h2).val == 1); //~ ERROR
}

pub struct Inner { pub x: i32 }
pub struct Outer { pub a: i32, pub inner: Inner }
pub struct H<'a> { r: &'a Inner }

#[pure]
#[trusted]
#[container_of(r, inner)]
pub fn get_outer<'a>(_h: &H<'a>) -> &'a Outer { unimplemented!() }

#[requires(get_outer(h1).a == 1)]
pub fn containers(h1: &H<'static>, h2: &H<'static>) {
    assert!(get_outer(h2).a == 1); //~ ERROR
}

pub struct G<'a> { pub c: &'a Cell<i32> }

impl<'a> G<'a> {
    pub fn bump(&mut self) { self.c.set(self.c.get() + 1); }
}

fn noop(_g: &mut G) { }

pub fn mutref_shared() {
    let c = Cell::new(0);
    let mut g = G { c: &c };
    g.bump();
    noop(&mut g);
    let _x = c.get();
    let _y = g.c.get();
}

pub fn mutref_shared_value() {
    let c = Cell::new(0);
    let mut g = G { c: &c };
    noop(&mut g);
    assert!(c.get() == 0); //~ ERROR
}

#[trusted]
#[requires(Rc::ptr_eq(a, &b))]
#[ensures(Rc::strong_count(a) == old(Rc::strong_count(a)))]
fn wrap(a: &Rc<i32>, b: Rc<i32>) -> ManuallyDrop<Rc<i32>> { ManuallyDrop::new(b) }

pub fn manually_drop() {
    let a = Rc::new(5);
    let b = Rc::clone(&a);
    let m = wrap(&a, b);
    assert!(Rc::strong_count(&a) == 2);
    drop(m);
    assert!(Rc::strong_count(&a) == 2);
    let _t = (wrap(&a, Rc::clone(&a)), String::new());
}

pub fn ctrl() { assert!(false); } //~ ERROR
