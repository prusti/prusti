use prusti_contracts::*;

// The result of a pure call is assigned its shallow snapshot; the value behind
// the mutable reference in it stays the one in the heap.
pub struct S<'a> {
    pub r: &'a mut i32,
}

#[pure]
fn pass<'a>(s: S<'a>) -> S<'a> {
    s
}

fn client(x: &mut i32) {
    *x = 3;
    let s = S { r: x };
    let t = pass(s);
    *t.r = 4;
    prusti_assert!(*t.r == 4);
}
