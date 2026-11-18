use prusti_contracts::*;

// Regression test for prusti/prusti#1409: derived comparison traits of a wrapper in specs.
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct Frame(usize);

fn comparisons() {
    prusti_assert!(Frame(0) < Frame(1));
    prusti_assert!(!(Frame(1) < Frame(1)));
    prusti_assert!(!(Frame(2) < Frame(1)));
    prusti_assert!(Frame(1) <= Frame(1));
    prusti_assert!(Frame(0) != Frame(1));
    prusti_assert!(Frame(3) == Frame(3));
}

fn from_fields(a: Frame, b: Frame) {
    if a.0 < b.0 {
        prusti_assert!(a < b);
    }
}
