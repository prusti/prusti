// Regression test for prusti/prusti#1161: inequality of references in pure functions.
use prusti_contracts::*;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AccountID(u32);

#[pure]
#[ensures(!result)]
pub fn check_eq(from: &AccountID) -> bool {
    from != from
}

#[pure]
pub fn not_equal(a: &AccountID, b: &AccountID) -> bool {
    a != b
}

pub fn copy_is_equal(a: &AccountID) {
    let b = *a;
    prusti_assert!(!not_equal(a, &b));
}
