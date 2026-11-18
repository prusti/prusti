use prusti_contracts::*;

// Regression test for prusti/prusti#65.
#[trusted]
#[requires(*x < 100)]
#[ensures(*x == 5)]
fn set5(x: &mut u32) {
    *x = 5;
}

#[requires(y < 100 && z < 100)]
fn wrong_branch(y: u32, z: u32, c: bool) {
    let mut y = y;
    let mut z = z;
    let mut x = &mut y;
    if c {
        x = &mut z;
    }
    set5(x);
    if c {
        prusti_assert!(y == 5); //~ERROR: the asserted expression might not hold
    }
}

#[requires(y < 100 && z < 100)]
fn wrong_unconditional(y: u32, z: u32, c: bool) {
    let mut y = y;
    let mut z = z;
    let mut x = &mut y;
    if c {
        x = &mut z;
    }
    set5(x);
    prusti_assert!(y == 5); //~ERROR: the asserted expression might not hold
}
