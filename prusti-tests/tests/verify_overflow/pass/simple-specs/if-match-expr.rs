//! Example: test if and match expressions

use prusti_contracts::*;

fn rand() -> bool { true }

#[requires(x == 42)]
#[ensures(if result > 0 { result == 42 } else { result == 0 })]
fn test_if_expr(x: u32) -> u32 {
    assert!(x == 42);
    if rand() {
        x
    } else {
        0
    }
}

#[requires(x == -42)]
#[ensures(match result { -84 => true, 123 | 456 => false, _ => false })]
fn test_match_expr(x: i32) -> i32 {
    assert!(x == -42);
    x * 2
}

#[requires(x == -42)]
#[ensures(match result { Some(k) => k == -42, _ => false })]
fn test_match_option_expr(x: i32) -> Option<i32> {
    assert!(x == -42);
    Some(x)
}

#[requires(x == -42)]
#[ensures(match result { Some(..) => true, _ => false })]
fn test_match_variants_expr(x: i32) -> Option<i32> {
    assert!(x == -42);
    Some(x)
}
