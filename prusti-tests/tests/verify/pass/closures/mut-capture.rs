//@ known-failure
// Calls of closures that capture references are not supported yet: a call goes
// to the generic stub of the `Fn*` trait method, which knows nothing about the
// closure (not even its `Output` type) and takes no permission for what it
// captures. Verification fails with an unbacktranslatable
// `call.precondition:insufficient.permission`.

pub fn straight_line() {
    let mut c = 0;
    let mut f = || c += 1;
    f();
    f();
}

// Closure capturing by `&mut` that is live across a loop.
pub fn map_with_counter() {
    let mut c = 0;
    for pair in [1, 2, 3].into_iter().map(|x| {
        c += 1;
        (x, c)
    }) {
        let _ = pair;
    }
}

// Closure capturing a reference that is called in a loop.
pub fn closure_in_loop(n: u32) {
    let k = 1u32;
    let r = &k;
    let g = |i: u32| i.wrapping_add(*r);
    let mut i = 0;
    while i < n { let _ = g(i); i += 1; }
}
