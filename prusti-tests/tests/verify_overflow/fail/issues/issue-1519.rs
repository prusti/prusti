use prusti_contracts::*;

// Regression test for prusti/prusti#1519.
fn external<'a>(t: &'a mut u32, r: &'a mut u32) -> &'a mut u32 {
    if *t == *r {
        t
    } else {
        r
    }
}

fn region_of_deref_probe() {
    let mut x = 1u32;
    let mut y = 1u32;
    let r: &mut u32 = external(&mut x, &mut y);
    *r += 0;
    x += *r - *r;
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

#[trusted]
#[ensures(*result == old(*t) || *result == old(*r))]
fn either<'a>(t: &'a mut u32, r: &'a mut u32) -> &'a mut u32 {
    t
}

fn wrong_claim() {
    let mut x = 1u32;
    let mut y = 2u32;
    let r = either(&mut x, &mut y);
    prusti_assert!(*r == 1); //~ERROR: the asserted expression might not hold
}
