//@ known-failure
//! Assignments to fields of local variables in pure code.
use prusti_contracts::*;

#[pure]
fn pure1() -> (u32, u32) {
    let mut x = (1, 2);
    x.1 = 3;
    x
}

fn client1() {
    let x = pure1();
    assert!(x == (1, 3));
}
