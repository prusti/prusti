use prusti_contracts::*;

// Regression test for prusti/prusti#1216.
#[ensures(*result == !old(*x))] //~ERROR: postcondition might not hold
pub fn assign_self_wrong(x: &mut bool) -> &mut bool {
    *x = *x;
    x
}

#[ensures(*result == old(*x))]
#[after_expiry(*x == before_expiry(*result))]
pub fn add_zero(x: &mut i32) -> &mut i32 {
    *x += 0;
    x
}

fn wrong_claim() {
    let mut v = 3;
    let r = add_zero(&mut v);
    *r = 7;
    prusti_assert!(v == 3); //~ERROR: the asserted expression might not hold
}
