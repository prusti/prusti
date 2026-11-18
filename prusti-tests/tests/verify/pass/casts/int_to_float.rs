use prusti_contracts::*;

fn i32_to_f32() {
    let x = 100;
    let y = x as f32;
    assert!(y == 100.0);
}

fn u8_to_f64() {
    let x = u8::MAX;
    let y = x as f64;
    assert!(y == 255.0);
}

fn negative_i32_to_f32() {
    let x = -5;
    let y = x as f32;
    assert!(y == -5.0);
}

fn i8_min_to_f32() {
    let x = i8::MIN;
    let y = x as f32;
    assert!(y == -128.0);
}

fn i64_to_f64() {
    let x = 0i64;
    let y = x as f64;
    assert!(y == 0.0);
}

fn max_u128() {
    let x = u128::MAX;
    let y = x as f32;
    prusti_assert!(y == f32::INFINITY);
}

fn rounding() {
    let x = 9007199254740993i64;
    prusti_assert!(x as f32 == 9007199254740992.0);
}

fn large_magnitude() {
    prusti_assert!(i128::MIN as f32 == -170141183460469231731687303715884105728.0);
}

#[requires(0 < x && x < 1000)]
fn symbolic(x: i32) {
    let y = x as f64;
    prusti_assert!(0.0 < y && y < 1000.0);
}
