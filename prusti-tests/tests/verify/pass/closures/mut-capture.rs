//@ known-failure
// Closures that capture by mutable reference are not supported yet: the PCG
// drops the `&mut` loan at the closure aggregate (it only creates borrow-flow
// edges for ADTs, tuples and arrays), and the closure type has no capture
// regions, so the closure's contract cannot refer to the captured places.
// Verification of `straight_line` fails with an unbacktranslatable "call.precondition:insufficient.permission".
// Verification of `map_with_counter` fails with an unsupported "a closure capturing a reference" error.

pub fn straight_line() {
    let mut c = 0;
    let mut f = || c += 1;
    f();
    f();
}

// Closure capturing by &mut that is live across a loop
pub fn map_with_counter() {
    let mut c = 0;
    for pair in [1, 2, 3].into_iter().map(|x| {
        c += 1;
        (x, c)
    }) {
        let _ = pair;
    }
}
