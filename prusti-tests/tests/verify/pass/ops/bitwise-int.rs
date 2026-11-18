use prusti_contracts::*;

fn main() {
    let x = 1;
    let y = 0;
    prusti_assert!(x & y == 0);

    let x = 1;
    let y = 4;
    prusti_assert!(x & y == 0);

    let x = 1;
    let y = 3;
    prusti_assert!(x & y == 1);

    let x = 1;
    let y = 3;
    prusti_assert!(x | y == 3);

    let x = 1;
    let y = 4;
    prusti_assert!(x | y == 5);

    let x = 1;
    let y = 0;
    prusti_assert!(x | y == 1);

    let x = 1;
    let y = 0;
    prusti_assert!(x ^ y == 1);

    let x = 1;
    let y = 1;
    prusti_assert!(x ^ y == 0);

    let x = 3;
    let y = 1;
    prusti_assert!(x ^ y == 2);

    let x = 1;
    let y = 2;
    prusti_assert!(x << y == 4);

    let x = 4;
    let y = 2;
    prusti_assert!(x >> y == 1);

    let x: u8 = 255;
    let y: u8 = 1;
    prusti_assert!(x << y == 254);

    let x: u8 = 128;
    let y: u8 = 1;
    prusti_assert!(x << y == 0);

    let x: u8 = 255;
    let y: u8 = 1;
    prusti_assert!(x >> y == 127);

    let x: i8 = -3;
    let y: i8 = 1;
    prusti_assert!(x << y == -6);
    prusti_assert!(x >> y == -2);

    let x: i8 = -4;
    prusti_assert!(x >> 1 == -2);
    let x: i8 = -16;
    prusti_assert!(x >> 3 == -2);
    let x: i8 = -128;
    prusti_assert!(x >> 7 == -1);
    let x: i8 = -1;
    prusti_assert!(x >> 5 == -1);
    let x: u8 = 252;
    prusti_assert!(x >> 1 == 126);

    let x = 0;
    let y = 1;
    prusti_assert!(x << 1 == 0);
    prusti_assert!(x >> 1 == 0);
}

#[requires(x >= -127 && x <= 127)]
#[ensures(result == x * 2)]
fn mul2(x: i16) -> i16 {
    x << 1
}

#[requires(x <= 127)]
#[ensures(result == x / 2)]
fn div2(x: u16) -> u16 {
    x >> 1
}

#[requires(x < 0)]
#[ensures(result < 0)]
fn neg_stays_neg(x: i8) -> i8 {
    x >> 1
}

#[ensures(x - result * 2 == 0 || x - result * 2 == 1)]
fn shr_is_floor_div(x: i8) -> i8 {
    x >> 1
}

#[ensures(result == u32::MAX - x)]
fn not_u32(x: u32) -> u32 {
    !x
}

#[ensures(result == -1 - x)]
#[ensures(!result == x)]
fn not_i64(x: i64) -> i64 {
    !x
}
