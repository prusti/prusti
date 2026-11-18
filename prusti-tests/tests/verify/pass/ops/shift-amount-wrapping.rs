//@ compile-flags: -Coverflow-checks=off

// Without overflow checks, a shift uses its amount modulo the bit width of the
// shifted value, whatever the type of the amount. The flag is needed because
// otherwise Prusti assumes rustc's overflow check that the amount is below the
// bit width.

use prusti_contracts::*;

fn amount_over_width() {
    let x: u8 = 1;
    let s: u32 = 9;
    prusti_assert!(x << s == 2);
}

fn negative_amount() {
    let x: u8 = 1;
    let s: i32 = -1;
    prusti_assert!(x << s == 128);
}

fn amount_wider_than_value() {
    let x: u8 = 0x80;
    let s: u128 = (1u128 << 64) + 3;
    prusti_assert!(x >> s == 0x10);
}

fn amount_narrower_than_value() {
    let x: u64 = 1;
    let s: u8 = 63;
    prusti_assert!(x << s == 0x8000_0000_0000_0000);
}
