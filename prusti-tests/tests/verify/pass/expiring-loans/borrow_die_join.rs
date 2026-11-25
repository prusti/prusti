//@ known-failure
use prusti_contracts::*;

// `y` and `z` are each moved out on one branch and lent out on the other, so
// after the join each holds a value only where it was lent out. The PCG does
// not track this yet, so this test is rejected with an internal error.

struct G {
    value: u32,
}

struct F {
    f: G,
    g: G,
}

fn consume_F(_f: F) {}

fn test6(y: F, z: F, b: bool) {
    let mut y = y;
    let mut z = z;
    let mut x;
    if b {
        x = &mut y;
        consume_F(z);
    } else {
        x = &mut z;
        consume_F(y);
    }
    let f = &mut x.f;
}
