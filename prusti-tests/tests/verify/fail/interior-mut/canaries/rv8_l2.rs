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

pub fn lib_like<T: Setter>(_log: &Cell<Option<*const T>>, x: &T) {
    x.poke()
}

pub fn user() {
    let log: Cell<Option<*const Cell<i32>>> = Cell::new(None);
    let c = Cell::new(1);
    lib_like(&log, &c);
    assert!(c.get() == 1); //~ ERROR
}

pub fn control() {
    let log: Cell<Option<*const Cell<i32>>> = Cell::new(None);
    let c = Cell::new(1);
    lib_like(&log, &c);
    assert!(false); //~ ERROR
}
