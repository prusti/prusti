use prusti_contracts::*;

#[requires(*a <= *b)]
#[ensures(*a == *b)] //~ ERROR: postcondition might not hold
fn le_is_not_eq<T: PartialOrd>(a: &T, b: &T) {}

// `x` may be NaN, which is not even less than or equal to itself.
#[ensures(x.le(&x))] //~ ERROR: postcondition might not hold
fn float_le_not_reflexive(x: f64) {}

// `max` returns `b` if `a` and `b` compare equal.
#[ensures(a.max(b) === a)] //~ ERROR: postcondition might not hold
fn max_is_not_first<T: Ord + Copy>(a: T, b: T) {}

fn clamp_panics() {
    let _ = 5i32.clamp(3, 1); //~ ERROR: precondition might not hold
}
