// Regression test for prusti/prusti#1262: matching a char reference against a char constant.
use prusti_contracts::*;

#[pure]
fn is_tab(c: &char) -> bool {
    matches!(c, '\t')
}

fn tab_is_not_tab() {
    let tab = '\t';
    prusti_assert!(!is_tab(&tab)); //~ ERROR: the asserted expression might not hold
}

fn a_is_tab() {
    let a = 'a';
    prusti_assert!(is_tab(&a)); //~ ERROR: the asserted expression might not hold
}
