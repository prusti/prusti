use prusti_contracts::*;

// TODO: calls of specified closures cannot be encoded yet, so the closures
// are only defined; their bodies are verified against their specifications.
fn main() {
    // Trusted: the body (which does not satisfy the specification) is
    // not verified.
    let f = closure!(
        #[trusted]
        #[ensures(result == x + 1)]
        |x: i32| -> i32 { x }
    );

    let g = closure!(
        #[pure]
        #[ensures(result == y + y)]
        |y: i32| -> i32 { y + y }
    );
}

// The body reads the capture `n` (the condition is there only for that): a
// user closure reads its captures from the closure value, unlike a
// quantifier's body.
fn capturing(n: i32) {
    let h = closure!(
        #[pure]
        #[ensures(result == y)]
        |y: i32| -> i32 { if n == n { y } else { 0 } }
    );
}
