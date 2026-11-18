//! Clients see the value after a field assignment in pure code; mutable
//! borrows and assignments to array elements are rejected.
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

// Calls to a pure function whose body is rejected still agree with each other.
#[pure]
fn pure2_caller() -> (u32, u32) {
    pure2()
}

fn client2_calls_agree() {
    let a = pure2();
    assert!(a == pure2());
    let b = pure2_caller();
    assert!(b == a);
}

#[pure]
fn pure3() -> [u32; 2] {
    let mut a = [1, 2];
    a[0] = 3; //~ ERROR: assignment to a part of a `[u32; 2]` in pure code
    a
}
