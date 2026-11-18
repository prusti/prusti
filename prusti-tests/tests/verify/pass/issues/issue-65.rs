// Regression test for prusti/prusti#65: passing a conditionally reborrowed `&mut` to a call.
use prusti_contracts::*;

#[trusted]
#[requires(*x < 100)]
#[ensures(*x == 5)]
fn set5(x: &mut u32) {
    *x = 5;
}

#[requires(y < 100 && z < 100)]
fn conditional_reborrow(y: u32, z: u32, c: bool) {
    let mut y = y;
    let mut z = z;
    let mut x = &mut y;
    if c {
        x = &mut z;
    }
    set5(x);
    if c {
        prusti_assert!(z == 5);
    } else {
        prusti_assert!(y == 5);
    }
}
