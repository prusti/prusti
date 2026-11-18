use prusti_contracts::*;

// Regression test for prusti/prusti#1409.
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct Frame(usize);

fn greater() {
    prusti_assert!(Frame(1) < Frame(0)); //~ ERROR: might not hold
}

fn irreflexive() {
    prusti_assert!(Frame(1) < Frame(1)); //~ ERROR: might not hold
}

fn distinct_equal() {
    prusti_assert!(Frame(0) == Frame(1)); //~ ERROR: might not hold
}

fn unrelated(a: Frame, b: Frame) {
    prusti_assert!(a < b); //~ ERROR: might not hold
}
