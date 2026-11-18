use prusti_contracts::*;

// Regression test for prusti/prusti#1216.
#[ensures(*result == old(*x))]
#[after_expiry(*x == before_expiry(*result))]
pub fn add_zero(x: &mut i32) -> &mut i32 {
    *x += 0;
    x
}

#[ensures(*result == old(*x))]
pub fn assign_self(x: &mut bool) -> &mut bool {
    *x = *x;
    x
}

fn caller() {
    let mut v = 3;
    let r = add_zero(&mut v);
    prusti_assert!(*r == 3);
    *r = 7;
    prusti_assert!(v == 7);
}
