use prusti_contracts::*;

#[ensures(result < 0.0)] //~ ERROR: postcondition might not hold
fn test_floats32_pos() -> f32 {
    return 1.0f32;
}

#[ensures(result > 0.0)] //~ ERROR: postcondition might not hold
fn test_floats32_neg() -> f32 {
    return -1.0f32;
}

#[ensures(result != 0.0)] //~ ERROR: postcondition might not hold
fn test_floats32_zero() -> f32 {
    return 0.0f32;
}

#[ensures(result < 0.0)] //~ ERROR: postcondition might not hold
fn test_floats64_pos() -> f64 {
    return 1.0f64;
}

#[ensures(result > 0.0)] //~ ERROR: postcondition might not hold
fn test_floats64_neg() -> f64 {
    return -1.0f64;
}

#[ensures(result != 0.0)] //~ ERROR: postcondition might not hold
fn test_floats64_zero() -> f64 {
    return 0.0f64;
}

#[requires(a < 0.0)]
#[ensures(result < 0.0)] //~ ERROR: postcondition might not hold
fn test_float_with_params(a: f32) -> f32{
    return -a;
}

// Float constants and preconditions on floats must not make the program
// inconsistent.

fn const_f32_consistent() {
    let _a = 0.1f32;
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

fn const_f64_consistent() {
    let _a = 0.5f64;
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

#[requires(a == 0.1)]
fn param_f32_consistent(a: f32) {
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

#[requires(a == 0.1)]
fn param_f64_consistent(a: f64) {
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

// Rounding the quotient 9.99... to 10 instead of truncating it would give 0.0.
#[requires(x == 1.0 && y == 0.1)]
#[ensures(result == 0.0)] //~ ERROR: postcondition might not hold
fn test_rem_rounded_f64(x: f64, y: f64) -> f64 {
    x % y
}

#[requires(x == 1.0 && y == 0.1)]
#[ensures(result == 0.0)] //~ ERROR: postcondition might not hold
fn test_rem_rounded_f32(x: f32, y: f32) -> f32 {
    x % y
}

#[requires(x == 4.5 && y == -3.0)]
#[ensures(result == -1.5)] //~ ERROR: postcondition might not hold
fn test_rem_sign_of_rhs(x: f64, y: f64) -> f64 {
    x % y
}

// `-1.0 % 1.0` is `-0.0`.
#[requires(x == -1.0 && y == 1.0)]
#[ensures(1.0 / result > 0.0)] //~ ERROR: postcondition might not hold
fn test_rem_pos_zero(x: f64, y: f64) -> f64 {
    x % y
}

#[requires(x == 1.0 && y == 0.1)]
fn test_rem_not_vacuous(x: f64, y: f64) {
    let r = x % y;
    prusti_assert!(r == 0.09999999999999995);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

#[requires(x == 1e20 && y == 3.0)]
fn test_rem_large_quotient_not_vacuous(x: f64, y: f64) {
    let r = x % y;
    prusti_assert!(!r.is_nan() && r >= 0.0 && r < 3.0);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

#[requires(!x.is_nan() && !x.is_infinite() && y.is_infinite())]
fn test_rem_inf_rhs_not_vacuous(x: f64, y: f64) {
    let r = x % y;
    prusti_assert!(r == x);
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}

#[requires((x == 1.0 && y == 0.0) || ((x.is_nan() || x.is_infinite()) && y == 2.0))]
fn test_rem_nan_not_vacuous(x: f64, y: f64) {
    let r = x % y;
    prusti_assert!(r.is_nan());
    prusti_assert!(false); //~ ERROR: the asserted expression might not hold
}
