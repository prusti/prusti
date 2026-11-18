use prusti_contracts::*;

// Regression test for prusti/prusti#1549.
fn test(x: u64, y: &u64) -> u64 {
    let y = *y;
    x / y //~ ERROR: attempt to divide by zero
}

fn call_zero() {
    test(1, &(0));
}

#[requires(*y == 5)]
fn needs_five(y: &u64) {}

fn call_wrong_const() {
    needs_five(&4); //~ ERROR: precondition might not hold
}
