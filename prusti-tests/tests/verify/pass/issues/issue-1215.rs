#![allow(unused_comparisons)]
use prusti_contracts::*;

// #[requires(0 <= i)] // This should not be needed, if encode_unsigned_num_constraint=true is the default
#[requires(i < s.len())]
fn _test(s: &[i64], i: usize) -> i64 {
    s[i]
}

fn _test_u8(x: u8) {
    prusti_assert!(x >= 0);
}

fn _test_u16(x: u16) {
    prusti_assert!(x >= 0);
}

fn _test_u32(x: u32) {
    prusti_assert!(x >= 0);
}

fn _test_u64(x: u64) {
    prusti_assert!(x >= 0);
}

fn _test_u128(x: u128) {
    prusti_assert!(x >= 0);
}

fn _test_usize(x: usize) {
    prusti_assert!(x >= 0);
}
