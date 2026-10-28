use prusti_contracts::*;

// The result of a pure call is a deep snapshot, and the destination is
// assigned its shallow snapshot: the state stays consistent.
pub struct S<'a> {
    pub r: &'a mut i32,
}

#[pure]
fn pass<'a>(s: S<'a>) -> S<'a> {
    s
}

fn client(x: &mut i32) {
    let s = S { r: x };
    let _t = pass(s);
    prusti_assert!(false); //~ ERROR: assertion might not hold
}

fn main() {}
