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
