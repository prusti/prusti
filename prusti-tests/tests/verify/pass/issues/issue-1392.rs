use prusti_contracts::*;

// Regression test for prusti/prusti#1392: snapshot (in)equality of integer literals.
fn literals_differ() {
    prusti_assert!(0 !== 1);
    prusti_assert!(0 === 0);
}
