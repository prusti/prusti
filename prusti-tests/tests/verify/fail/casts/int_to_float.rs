use prusti_contracts::*;

fn exact() {
    let x = 100;
    let y = x as f32;
    prusti_assert!(y == 101.0); //~ERROR: the asserted expression might not hold
}

fn rounding() {
    let x = 9007199254740993i64;
    // The next `f32` after the correctly rounded result.
    prusti_assert!(x as f32 == 9007200328482816.0); //~ERROR: the asserted expression might not hold
}

fn max_u128() {
    let x = u128::MAX;
    let y = x as f32;
    prusti_assert!(y != f32::INFINITY); //~ERROR: the asserted expression might not hold
}

#[requires(0 < x && x < 1000)]
fn symbolic(x: i32) {
    let y = x as f64;
    prusti_assert!(1.0 < y); //~ERROR: the asserted expression might not hold
}
