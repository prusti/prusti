//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::cell::Cell;

pub fn two_box_cells() {
    let a: Box<Cell<i32>> = Default::default();
    let b: Box<Cell<i32>> = Default::default();
    assert!(false); //~ ERROR
}

pub fn one_box_cell() {
    let a: Box<Cell<i32>> = Default::default();
    assert!(false); //~ ERROR
}

pub fn set_one() {
    let a: Box<Cell<i32>> = Default::default();
    let b: Box<Cell<i32>> = Default::default();
    a.set(5);
    assert!(b.get() == 5); //~ ERROR
}
