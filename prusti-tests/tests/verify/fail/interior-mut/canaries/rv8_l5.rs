//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub trait Setter {
    fn poke(&self);
}

impl Setter for Cell<i32> {
    fn poke(&self) {
        self.set(5);
    }
}

pub struct Holder<X>(pub X);

impl Holder<Cell<i32>> {
    pub fn visit<U: Setter>(&self, u: &U) {
        u.poke()
    }
}

pub fn user() {
    let a = Holder(Cell::new(0));
    let c = Cell::new(1);
    a.visit(&c);
    assert!(c.get() == 1); //~ ERROR
}

pub fn control() {
    let a = Holder(Cell::new(0));
    let c = Cell::new(1);
    a.visit(&c);
    assert!(false); //~ ERROR
}
