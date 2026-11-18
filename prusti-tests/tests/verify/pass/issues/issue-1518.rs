// Regression test for prusti/prusti#1518: several shared borrows of the same local.
use prusti_contracts::*;

fn reborrows_equal() {
    let x: u8 = 5;
    let r = &x;
    let rb = &*r;
    let s = &x;
    let rs = &*s;
    prusti_assert!(rb == rs);
    prusti_assert!(*rb == 5);
}
