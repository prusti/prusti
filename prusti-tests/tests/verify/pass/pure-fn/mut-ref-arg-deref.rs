use prusti_contracts::*;

pub struct S {
    v: i32,
}

// A pure function reads the current value behind a mutable reference argument.
#[pure]
fn get(s: &mut S) -> i32 {
    s.v
}

fn client(s: &mut S) {
    s.v = 3;
    let v1 = get(s);
    prusti_assert!(v1 == 3);
    s.v = 4;
    let v2 = get(s);
    prusti_assert!(v2 == 4);
}
