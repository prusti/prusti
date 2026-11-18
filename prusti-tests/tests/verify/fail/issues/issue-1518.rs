use prusti_contracts::*;

// Regression test for prusti/prusti#1518.
fn foo(a: &u8, b: &u8) {}

fn two_shared_refs() {
    let x: u8 = 5;
    foo(&x, &x);
    prusti_assert!(false); //~ERROR: the asserted expression might not hold
}

fn reborrows() {
    let x: u8 = 5;
    let r = &x;
    let rb = &*r;
    let s = &x;
    let rs = &*s;
    prusti_assert!(rb == rs);
    prusti_assert!(false); //~ERROR: the asserted expression might not hold
}
