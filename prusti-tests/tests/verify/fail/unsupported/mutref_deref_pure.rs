use prusti_contracts::*;

pub struct S {
    v: i32,
}

// A mutable reference argument's snapshot is deep, so a pure function can read
// the value behind it, but not the value behind a mutable reference behind it:
// that referent is read through the snapshot of a type parameter, which is
// shallow.

#[pure]
fn get(s: &mut S) -> i32 {
    s.v
}

#[pure]
fn get_nested(s: &mut &mut S) -> i32 { //~ ERROR: the result of pure function `get_nested` might not match its definition
    s.v //~ERROR: dereference of the mutable reference `&mut S` in pure code
}

fn client(s: &mut S, t: &mut &mut S) {
    let _ = get(s);
    let _ = get_nested(t);
}
