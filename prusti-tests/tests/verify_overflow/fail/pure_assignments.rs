//@ known-failure
//! Mutable borrows and assignments to array elements are not supported in
//! pure code.
use prusti_contracts::*;

#[pure]
fn pure1() -> (u32, u32) {
    let mut x = (1, 2);
    x.1 = 3;
    x
}

fn client1() {
    assert!(pure1() == (1, 2)); //~ ERROR: might not hold
}

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

#[pure]
fn pure3() -> [u32; 2] {
    let mut a = [1, 2];
    a[0] = 3; //~ ERROR: assignment into a value of type `[u32; 2]` in pure code
    a
}
