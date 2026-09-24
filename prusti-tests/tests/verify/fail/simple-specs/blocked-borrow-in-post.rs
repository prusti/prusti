use prusti_contracts::*;

#[pure]
fn read(x: &i32) -> i32 {
    *x
}

// At the end of these functions `*x` is blocked by the returned reborrow, so
// the postconditions cannot read it.

#[ensures(*x == 0)] //~ERROR: this specification requires memory permissions that are not available
fn deref(x: &mut i32) -> &mut i32 {
    x
}

#[ensures(read(x) == 0)] //~ERROR: this specification requires memory permissions that are not available
fn pure_call(x: &mut i32) -> &mut i32 {
    x
}
