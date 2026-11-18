// Regression test for prusti/prusti#1161: inequality of references in pure functions.
use prusti_contracts::*;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AccountID(u32);

#[pure]
pub fn not_equal(a: &AccountID, b: &AccountID) -> bool {
    a != b
}

pub fn arbitrary_are_equal(a: &AccountID, b: &AccountID) {
    prusti_assert!(!not_equal(a, b)); //~ ERROR: the asserted expression might not hold
}

pub fn arbitrary_are_different(a: &AccountID, b: &AccountID) {
    prusti_assert!(not_equal(a, b)); //~ ERROR: the asserted expression might not hold
}
