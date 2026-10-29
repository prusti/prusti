use prusti_contracts::*;

pub fn simple_loop() {
    let mut x = 0;
    while x < 100 {
        body_invariant!(x < 100);
        x += 1;
    }
}

fn test_loop(b: bool) {
    let mut g = b;
    while g {
        body_invariant!(g); // Ok, as the loop invariant is not reached after `g = false`
        g = false;
    }
}

fn test1() {
    let mut i = 0;
    while i < 10 {
        body_invariant!(i < 10);
        i += 1;
    }
    assert!(i == 10);
}

#[ensures(result == (old(*i) >= 0))]
#[ensures(*i == 1 + old(*i))]
fn test_and_increment(i: &mut usize) -> bool {
    let old_i = *i;
    *i += 1;
    old_i >= 0
}

#[requires(*i > 0)]
fn work(i: &mut usize) {
    // ...
}

fn client() {
    let mut i = 0;
    while test_and_increment(&mut i) {
        body_invariant!(i > 0);
        work(&mut i);
    }
    assert!(i <= 0);
}

fn write_through_mut_ref() {
    let mut i = 0;
    let mut x = 0;
    while i < 3 {
        body_invariant!(0 <= i && i < 3);
        let r = &mut x;
        *r += 1;
        i += 1;
    }
}

fn write_through_mut_ref_after_branch(b: bool) {
    let mut i = 0;
    let mut x = 0;
    while i < 3 {
        if b {
            i += 1;
        }
        body_invariant!(0 <= i && i <= 3);
        let r = &mut x;
        *r += 1;
        if i == 3 {
            break;
        }
        i += 1;
    }
}
