use prusti_contracts::*;

struct S {
    f: i32,
}

fn main() {
    let mut s = S { f: 1 };
    let r = &mut s;
    let p = &raw mut (*r).f;
    *r = S { f: 2 };
    let y = unsafe { *p };
    prusti_assert!(y == 1); //~ ERROR: the asserted expression might not hold
}
