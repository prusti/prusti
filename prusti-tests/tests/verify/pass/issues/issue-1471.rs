use prusti_contracts::*;

// Regression test for prusti/prusti#1471.
fn reassign_argument<'a>(mut x: &'a mut i32, y: &'a mut i32) {
    x = y;
}

fn write_after_reassign<'a>(mut x: &'a mut i32, y: &'a mut i32) {
    x = y;
    *x = 7;
    prusti_assert!(*x == 7);
}

#[ensures(*y == 5)]
fn postcondition_on_reassigned<'a>(mut x: &'a mut i32, y: &'a mut i32) {
    x = y;
    *x = 5;
}

fn caller() {
    let mut a = 1;
    let mut b = 2;
    postcondition_on_reassigned(&mut a, &mut b);
    prusti_assert!(b == 5);
}
