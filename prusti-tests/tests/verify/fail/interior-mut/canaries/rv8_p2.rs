//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub trait Get {
    #[pure]
    fn g(&self) -> i32;
    fn poke(&self);
}

#[refine_trait_spec]
impl Get for Cell<i32> {
    #[pure]
    fn g(&self) -> i32 { self.get() }
    fn poke(&self) { self.set(5) }
}

pub fn h<T: Get>(x: &T) {
    let a = x.g();
    x.poke();
    let b = x.g();
    assert!(a == b); //~ ERROR
}

pub fn h_control<T: Get>(x: &T) {
    let a = x.g();
    x.poke();
    let b = x.g();
    assert!(false); //~ ERROR
}

pub fn user() {
    let c = Cell::new(1);
    h(&c); // panics at runtime: a == 1, b == 5
}
