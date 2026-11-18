// Regression test for prusti/prusti#22: returning a shared reborrow of a field of a `&mut`
// argument.
use prusti_contracts::*;

struct Point {
    x: u32,
    y: u32,
}

#[ensures(*result == old(p.x))]
#[after_expiry(p.x == old(p.x) && p.y == old(p.y))]
fn reborrow_x<'a>(p: &'a mut Point) -> &'a u32 {
    &p.x
}

#[after_expiry(p.x == old(p.x))]
fn reborrow_y<'a>(p: &'a mut Point) -> &'a u32 {
    &p.y
}

#[requires(p.x == 5 && p.y == 6)]
fn caller(p: &mut Point) {
    let r = reborrow_x(p);
    let v = *r;
    assert!(v == 5);
    assert!(p.y == 6);
    assert!(p.x == 5);
}
