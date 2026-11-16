use prusti_contracts::*;

#[requires(f == 2.0)]
#[ensures(result == f)]
fn foo(f: f32) -> f32 {
    f
}

#[requires(f == 2.0)]
#[ensures(result == 2.5)]
fn foo2(f: f32) -> f32 {
    f + 0.5
}

#[requires(f == 4.5)]
#[ensures(result == 1.5)]
fn foo3(f: f32) -> f32 {
    f % 3.0
}

#[requires(f == 4.25)]
#[ensures(result == 1.25)]
fn foo4(f: f32) -> f32 {
    f % 3.0
}

// `%` is computed exactly: the quotient 9.99... is truncated to 9, not
// rounded to 10.
#[requires(x == 1.0 && y == 0.1)]
#[ensures(result == 0.09999999999999995)]
fn rem_exact_f64(x: f64, y: f64) -> f64 {
    x % y
}

#[requires(x == 1.0 && y == 0.1)]
#[ensures(result == 0.09999999)]
fn rem_exact_f32(x: f32, y: f32) -> f32 {
    x % y
}

// For a quotient too large for the significand, only bounds on the result are
// known.
#[requires(x == 1e20 && y == 3.0)]
#[ensures(!result.is_nan() && result >= 0.0 && result < 3.0)]
fn rem_large_quotient(x: f64, y: f64) -> f64 {
    x % y
}

// The result has the sign of `x`.
#[requires(x == -4.5 && y == 3.0)]
#[ensures(result == -1.5)]
fn rem_neg_lhs(x: f64, y: f64) -> f64 {
    x % y
}

#[requires(x == 4.5 && y == -3.0)]
#[ensures(result == 1.5)]
fn rem_neg_rhs(x: f64, y: f64) -> f64 {
    x % y
}

// A zero result has the sign of `x` too (`1.0 / -0.0` is negative).
#[requires(x == -1.0 && y == 1.0)]
#[ensures(result == 0.0 && 1.0 / result < 0.0)]
fn rem_neg_zero(x: f64, y: f64) -> f64 {
    x % y
}

#[requires(x == 2.0 && y == -1.0)]
#[ensures(result == 0.0 && 1.0 / result > 0.0)]
fn rem_pos_zero(x: f64, y: f64) -> f64 {
    x % y
}

#[requires(!x.is_nan() && !x.is_infinite() && y.is_infinite())]
#[ensures(result == x)]
fn rem_inf_rhs(x: f64, y: f64) -> f64 {
    x % y
}

#[requires((x == 1.0 && y == 0.0) || ((x.is_nan() || x.is_infinite()) && y == 2.0))]
#[ensures(result.is_nan())]
fn rem_nan(x: f64, y: f64) -> f64 {
    x % y
}

#[requires(!f32_is_nan(f))]
#[ensures(result == f)]
fn foo5(f: f32) -> f32 {
    f
}

#[requires(!f.is_nan())]
#[ensures(result == f)]
fn foo6(f: f64) -> f64 {
    f
}

#[requires(!x.is_nan())]
#[requires(!y.is_nan())]
#[ensures(result == if x > y { 4 } else { 2 })]
fn foo7(x: f32, y: f32) -> u8 {
    if y < x {
        4
    } else {
        2
    }
}

#[requires(!x.is_nan())]
#[requires(!y.is_nan())]
#[requires(x != y)]
#[ensures(!result)]
fn foo8(x: f32, y: f32) -> bool {
    x == 2.5 && y == 2.5
}

#[requires(x == 3.3)]
#[ensures(result == -3.3)]
fn foo9(x: f32) -> f32 {
    -x
}

#[requires(!x.is_nan())]
#[requires(!x.is_infinite())]
#[ensures(!result.is_infinite())]
fn foo10(x: f32) -> f32 {
    x
}

#[requires(x == 4.5)]
#[ensures((result - (2.0 * x)).abs() <= 0.01)]
fn times_two(x: f32) -> f32 {
    x + x
}
