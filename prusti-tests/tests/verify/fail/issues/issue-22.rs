// Regression test for prusti/prusti#22: returning a shared reborrow of a field of a `&mut`
// argument.
use prusti_contracts::*;

struct Point {
    x: u32,
    y: u32,
}

#[after_expiry(p.y == old(p.y) + 1)] //~ERROR: pledge in the postcondition might not hold
fn wrong_pledge<'a>(p: &'a mut Point) -> &'a u32 {
    &p.x
}

#[ensures(*result == old(p.x))]
#[after_expiry(p.x == old(p.x) && p.y == old(p.y))]
fn reborrow_x<'a>(p: &'a mut Point) -> &'a u32 {
    &p.x
}

#[requires(p.x == 5 && p.y == 6)]
fn wrong_claim_after_reborrow(p: &mut Point) {
    let r = reborrow_x(p);
    let _v = *r;
    prusti_assert!(p.y == 7); //~ERROR: the asserted expression might not hold
}
