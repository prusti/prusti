//@ known-failure
//! Mutable borrows are not supported in pure code.
use prusti_contracts::*;

#[pure]
fn pure2() -> (u32, u32) {
    let mut x = (1, 2);
    let y = &mut x.1; //~ERROR: mutable borrow of `u32` in a specification
    *y = 3;
    x
}

fn client2() {
    let x = pure2();
    let y = (1, 2);
    assert!(x == y); //~ ERROR: might not hold
}
