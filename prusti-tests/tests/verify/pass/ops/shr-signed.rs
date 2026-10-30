use prusti_contracts::*;

fn main() {
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
}

#[requires(x < 0)]
#[ensures(result < 0)]
fn neg_stays_neg(x: i8) -> i8 {
    x >> 1
}

#[ensures(result == x / 2 || result == x / 2 - 1)]
fn shr_is_floor_div(x: i8) -> i8 {
    x >> 1
}
