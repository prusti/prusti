//@ known-failure
// Closures that capture by mutable reference are not supported yet: the PCG
// drops the `&mut` loan at the closure aggregate (it only creates borrow-flow
// edges for ADTs, tuples and arrays), and the closure type has no capture
// regions, so the closure's contract cannot refer to the captured places.
// Verification fails with an unbacktranslatable `call.precondition:insufficient.permission`.

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

// This fails with `a closure capturing a reference` instead of an
// unbacktranslatable `call.precondition:insufficient.permission` error.
pub fn closure_in_loop(n: u32) {
    let k = 1u32;
    let r = &k;
    let g = |i: u32| i.wrapping_add(*r);
    let mut i = 0;
    while i < n { let _ = g(i); i += 1; }
}
