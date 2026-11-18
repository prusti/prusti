use prusti_contracts::*;

// Regression test for prusti/prusti#1518.
fn reborrows_equal() {
    let mut x: u8 = 5;
    let r = &x;
    let rb = &*r;
    let s = &x;
    let rs = &*s;
    prusti_assert!(rb == rs);
    prusti_assert!(*rb == 5);
}
