// Regression test for prusti/prusti#1519: a `&mut` result that may borrow from either of two
// arguments, with overflow checks on as in the issue.
use prusti_contracts::*;

fn external<'a>(t: &'a mut u32, r: &'a mut u32) -> &'a mut u32 {
    if *t == *r {
        t
    } else {
        r
    }
}

fn region_of_deref() {
    let mut x = 1u32;
    let mut y = 1u32;
    let r: &mut u32 = external(&mut x, &mut y);
    *r += 0;
    x += *r - *r;
}

#[trusted]
#[ensures(*result == old(*t) || *result == old(*r))]
fn either<'a>(t: &'a mut u32, r: &'a mut u32) -> &'a mut u32 {
    t
}

fn caller_of_either() {
    let mut x = 1u32;
    let mut y = 2u32;
    let r = either(&mut x, &mut y);
    assert!(*r == 1 || *r == 2);
}
