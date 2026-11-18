// Regression test for prusti/prusti#1392: snapshot (in)equality of integer literals.
use prusti_contracts::*;

fn literals_equal() {
    prusti_assert!(0 === 1); //~ ERROR: might not hold
}

fn literal_unequal_to_itself() {
    prusti_assert!(0 !== 0); //~ ERROR: might not hold
}
