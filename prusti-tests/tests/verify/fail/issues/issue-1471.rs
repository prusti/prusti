use prusti_contracts::*;

// Regression test for prusti/prusti#1471.
fn wrong_value<'a>(mut x: &'a mut i32, y: &'a mut i32) {
    x = y;
    *x = 7;
    prusti_assert!(*x == 8); //~ERROR: the asserted expression might not hold
}

#[ensures(*y == 6)] //~ERROR: postcondition might not hold
fn wrong_postcondition<'a>(mut x: &'a mut i32, y: &'a mut i32) {
    x = y;
    *x = 5;
}

#[ensures(*y == 5)]
fn postcondition_on_reassigned<'a>(mut x: &'a mut i32, y: &'a mut i32) {
    x = y;
    *x = 5;
}

fn wrong_caller_claim() {
    let mut a = 1;
    let mut b = 2;
    postcondition_on_reassigned(&mut a, &mut b);
    prusti_assert!(b == 6); //~ERROR: the asserted expression might not hold
}
