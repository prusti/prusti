use prusti_contracts::*;

// A struct with a lifetime parameter but no mutable reference can be passed to
// a pure function from any place, e.g. a local built in the impure code.
#[derive(Clone, Copy)]
pub struct W<'a> {
    pub r: &'a i32,
}

#[pure]
fn get_w(w: W<'_>) -> i32 {
    *w.r
}

#[pure]
#[ensures(result == *x)]
fn wrap_get(x: &i32) -> i32 {
    get_w(W { r: x })
}

fn client() {
    let x = 5;
    let w = W { r: &x };
    prusti_assert!(get_w(w) == 5);
    let y = wrap_get(&x);
    prusti_assert!(y == 5);
}
