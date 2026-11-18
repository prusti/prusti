// Regression test for prusti/prusti#619: derived trait impls.
use prusti_contracts::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct P {
    pub x: i32,
}

pub fn derived_eq() {
    let p = P { x: 1 };
    let q = P { x: 2 };
    prusti_assert!(p == q); //~ ERROR: might not hold
}

pub fn derived_probe() {
    let p = P { x: 1 };
    prusti_assert!(p == P { x: 1 });
    prusti_assert!(false); //~ ERROR: might not hold
}
