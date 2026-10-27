//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

// Stands for a type from another crate: this crate's bodies never name `Cell`.
pub struct Wrapper { c: Cell<i32> }

// Stands for `w.c.set(w.c.get() + 1)` in the other crate.
#[trusted]
pub fn bump_impl(w: &Wrapper) { unimplemented!() }

pub trait Tr { fn bump(&self); }
impl Tr for Wrapper {
    fn bump(&self) { bump_impl(self) }
}

#[trusted]
pub fn mk() -> Wrapper { unimplemented!() }

#[pure]
#[trusted]
pub fn val(w: &Wrapper) -> i32 { unimplemented!() }

fn g<T: Tr>(x: &T) { x.bump() }

pub fn direct() {
    let w = mk();
    let a = val(&w);
    w.bump();
    assert!(val(&w) == a); //~ ERROR
}

pub fn through_generic() {
    let w = mk();
    let a = val(&w);
    g(&w);
    assert!(val(&w) == a); //~ ERROR
}

pub fn control() {
    let w = mk();
    let a = val(&w);
    g(&w);
    assert!(false); //~ ERROR
}
