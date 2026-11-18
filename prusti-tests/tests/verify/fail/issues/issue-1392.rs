use prusti_contracts::*;

// Regression test for prusti/prusti#1392.
fn literals_equal() {
    prusti_assert!(0 === 1); //~ ERROR: might not hold
}

fn literal_unequal_to_itself() {
    prusti_assert!(0 !== 0); //~ ERROR: might not hold
}
