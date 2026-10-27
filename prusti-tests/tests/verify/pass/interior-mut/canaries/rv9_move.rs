//@ compile-flags: -Passert_timeout=45000
use prusti_contracts::cell::Cell;

pub fn move_none() {
    let a: Option<Cell<i32>> = None;
    let b = a;
    assert!(b.is_none());
}
