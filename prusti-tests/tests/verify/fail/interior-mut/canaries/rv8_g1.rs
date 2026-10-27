//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::*;
use prusti_contracts::cell::Cell;

pub fn two<T>(_a: &T, _b: &T) {}

pub fn alias() {
    let c = Cell::new(1);
    two(&c, &c);
}

pub fn distinct() {
    let c = Cell::new(1);
    let d = Cell::new(1);
    two(&c, &d);
    assert!(c.get() == 1); //~ ERROR
}

pub fn control() {
    assert!(false); //~ ERROR
}
